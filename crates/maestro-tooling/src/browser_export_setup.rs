//! Prepare the temporary browser export library from the checkout's current manifests.

use std::{collections::HashSet, ffi::OsString, fs, io, path::Path, process::ExitCode};

use toml::{Table, Value};

use crate::cargo_lock::Lock;

/// Dispatch binding-version, manifest preparation or external lock comparison.
///
/// # Errors
/// Returns missing operands, malformed manifest/lock data, version differences or I/O failures.
pub(crate) fn run(args: &[OsString], checkout: &Path) -> io::Result<ExitCode> {
    let value = args
        .get(1)
        .ok_or_else(|| io::Error::other("missing binding version or output parameter"))?;
    match args.first().and_then(|arg| arg.to_str()) {
        Some("binding-version") => {
            let lock = Lock::read(checkout)?;
            let expected = format!("wasm-bindgen {}", lock.version("wasm-bindgen")?);
            if value.to_str().map(str::trim) != Some(expected.as_str()) {
                return Err(io::Error::other(format!(
                    "wasm-bindgen CLI must match Cargo.lock: expected {expected}"
                )));
            }
        }
        Some("prepare") => prepare(checkout, Path::new(value))?,
        Some("check-lock") => {
            Lock::read(checkout)?.check_external_identities(&Lock::read(Path::new(value))?)?;
            println!("LOCK external identity additions: 0");
        }
        _ => return Err(io::Error::other("unknown browser export setup operation")),
    }
    Ok(ExitCode::SUCCESS)
}

/// Write a standalone cdylib manifest beside the copied lock and linked authored tests.
pub(crate) fn prepare(checkout: &Path, output: &Path) -> io::Result<()> {
    let checkout = maestro_realpath::canonicalize(checkout)?;
    let root = checkout
        .to_str()
        .ok_or_else(|| io::Error::other("browser preparation requires UTF-8 paths"))?;
    let manifest = manifest(root).map_err(|error| {
        io::Error::other(format!("browser export Cargo.toml preparation: {error}"))
    })?;
    fs::create_dir_all(output)?;
    fs::write(output.join("Cargo.toml"), manifest)?;
    for file in ["Cargo.lock", "rust-toolchain.toml"] {
        fs::copy(checkout.join(file), output.join(file))?;
    }
    let tests = checkout.join("crates/maestro-web/tests");
    #[cfg(unix)]
    std::os::unix::fs::symlink(tests, output.join("tests"))?;
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(tests, output.join("tests"))?;
    Ok(())
}

/// Derive browser dependencies and root patches without changing authored source or locks.
fn manifest(root: &str) -> io::Result<String> {
    let workspace = read_manifest(&maestro_path::join(&[root, "Cargo.toml"]))?;
    let member_root = maestro_path::join(&[root, "crates/maestro-web"]);
    let member = read_manifest(&maestro_path::join(&[&member_root, "Cargo.toml"]))?;
    let mut dependencies = Table::new();
    let wasm = member
        .get("target")
        .map(table)
        .transpose()?
        .and_then(|target| target.get("cfg(target_arch = \"wasm32\")"))
        .map(table)
        .transpose()?;
    for section in [
        member.get("dependencies"),
        wasm.and_then(|target| target.get("dependencies")),
        member.get("dev-dependencies"),
    ] {
        let Some(section) = section else { continue };
        for (name, value) in table(section)? {
            let mut resolved = dependency(name, value, &workspace, &member_root, root)?;
            if let Some(previous) = dependencies.get(name) {
                union_features(&mut resolved, table(previous)?)?;
            }
            dependencies.insert(name.clone(), resolved.into());
        }
    }
    let own = dependencies
        .entry("maestro-web")
        .or_insert_with(|| Value::Table(Table::new()));
    own.as_table_mut()
        .ok_or_else(|| io::Error::other("invalid maestro-web dependency"))?
        .insert("path".into(), member_root.clone().into());
    let mut result: Table = toml::from_str("[package]\nname='maestro-browser-test'\nversion='0.0.0'\nedition='2024'\npublish=false\n[workspace]\n[lib]\ncrate-type=['cdylib']\n").map_err(io::Error::other)?;
    result
        .get_mut("lib")
        .and_then(Value::as_table_mut)
        .ok_or_else(|| io::Error::other("missing browser library table"))?
        .insert(
            "path".into(),
            maestro_path::join(&[&member_root, "tests/integration.rs"]).into(),
        );
    result.insert("dependencies".into(), dependencies.into());
    if let Some(profile) = profile(&workspace, root)? {
        result.insert("profile".into(), profile.into());
    }
    if let Some(patches) = workspace.get("patch") {
        result.insert("patch".into(), absolute_patches(patches, root)?.into());
    }
    toml::to_string(&result).map_err(io::Error::other)
}

/// Copy the root profiles so the library's dependencies compile exactly as the checkout's do.
///
/// A package override that names nothing the library builds is dropped: Cargo warns about it.
fn profile(workspace: &Table, root: &str) -> io::Result<Option<Table>> {
    let Some(mut profile) = workspace.get("profile").map(table).transpose()?.cloned() else {
        return Ok(None);
    };
    let lock = Lock::read(Path::new(root))?;
    let built: HashSet<&str> = lock
        .graph(|_| true)
        .closure(["maestro-web"])
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    for (_, settings) in &mut profile {
        let Some(settings) = settings.as_table_mut() else {
            continue;
        };
        if let Some(packages) = settings.get_mut("package").and_then(Value::as_table_mut) {
            let unused: Vec<String> = packages
                .keys()
                .filter(|name| *name != "*" && !built.contains(name.as_str()))
                .cloned()
                .collect();
            for name in unused {
                packages.remove(&name);
            }
        }
    }
    Ok(Some(profile))
}

/// Anchor every registry's replacement dependencies to the root manifest.
fn absolute_patches(patches: &Value, root: &str) -> io::Result<Table> {
    let mut registries = Table::new();
    for (registry, patches) in table(patches)? {
        let mut resolved = Table::new();
        for (name, value) in table(patches)? {
            resolved.insert(name.clone(), absolute(value, root)?.into());
        }
        registries.insert(registry.clone(), resolved.into());
    }
    Ok(registries)
}

/// Parse a TOML document with its authored filename in the diagnostic.
fn read_manifest(path: &str) -> io::Result<Table> {
    let text = fs::read_to_string(path)?;
    toml::from_str(&text).map_err(|error| io::Error::other(format!("{path}: {error}")))
}

/// Require a dependency section or value to be a table.
fn table(value: &Value) -> io::Result<&Table> {
    value
        .as_table()
        .ok_or_else(|| io::Error::other("expected a dependency table"))
}

/// Resolve workspace inheritance by the dependency's authored alias, retaining member overrides.
fn dependency(
    name: &str,
    value: &Value,
    workspace: &Table,
    base: &str,
    root: &str,
) -> io::Result<Table> {
    let mut member = absolute(value, base)?;
    let inherited = member.remove("workspace");
    match inherited {
        None | Some(Value::Boolean(false)) => Ok(member),
        Some(Value::Boolean(true)) => {
            let value = workspace
                .get("workspace")
                .and_then(|value| value.get("dependencies"))
                .and_then(|value| value.get(name))
                .ok_or_else(|| io::Error::other(format!("missing workspace dependency {name}")))?;
            let mut resolved = absolute(value, root)?;
            union_features(&mut member, &resolved)?;
            resolved.extend(member);
            Ok(resolved)
        }
        Some(_) => Err(io::Error::other("dependency workspace must be a boolean")),
    }
}

/// Convert shorthand versions and anchor dependency paths to their authoring directory.
fn absolute(value: &Value, base: &str) -> io::Result<Table> {
    let mut dependency = if let Some(version) = value.as_str() {
        Table::from_iter([("version".to_owned(), version.into())])
    } else {
        table(value)?.clone()
    };
    if let Some(path) = dependency.get_mut("path") {
        let relative = path
            .as_str()
            .ok_or_else(|| io::Error::other("dependency path must be a string"))?;
        *path = maestro_path::resolve(
            &[base, relative],
            &maestro_path::Cwd {
                current: base,
                drive_directories: &[],
            },
        )
        .into();
    }
    if dependency.contains_key("features") {
        features(&dependency)?;
    }
    Ok(dependency)
}

/// Read declared dependency features, rejecting non-string entries.
fn features(dependency: &Table) -> io::Result<Vec<&str>> {
    let Some(value) = dependency.get("features") else {
        return Ok(Vec::new());
    };
    value
        .as_array()
        .ok_or_else(|| io::Error::other("dependency features must be an array"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| io::Error::other("dependency feature must be a string"))
        })
        .collect()
}

/// Combine inherited or earlier-section features in first-observed order without duplicates.
fn union_features(dependency: &mut Table, previous: &Table) -> io::Result<()> {
    let mut combined: Vec<String> = Vec::new();
    for feature in features(previous)?.into_iter().chain(features(dependency)?) {
        if !combined.iter().any(|value| value == feature) {
            combined.push(feature.to_owned());
        }
    }
    if !combined.is_empty() {
        dependency.insert("features".into(), combined.into());
    }
    Ok(())
}
