//! The workspace crates a tooling fixture must copy, read from Cargo metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use cargo_metadata::{Metadata, MetadataCommand, Package};

use crate::toolchain_support;

/// The crate every fixture replaces with an empty stand-in, so its sources are never copied.
const STAND_IN: &str = "maestro-dependencies";

/// The local crates the tooling package needs beside itself.
pub fn tooling_local_dependencies() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    local_dependencies(&workspace_metadata(&root), "maestro-tooling")
}

/// Read the packages this workspace declares, resolving no registry dependency.
fn workspace_metadata(root: &Path) -> Metadata {
    MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .exec()
        .unwrap()
}

/// The local crates `package` needs beside itself in `metadata`.
///
/// The copied root manifest makes every copied crate a workspace member, so cargo reads every
/// reached crate's own dependencies: development and build ones, every target's included. The
/// package itself stays out, so a crate that reaches it again cannot copy it over itself.
fn local_dependencies(metadata: &Metadata, package: &str) -> Vec<String> {
    let packages: BTreeMap<&str, &Package> = metadata
        .packages
        .iter()
        .filter_map(|package| Some((package.manifest_path.parent()?.file_name()?, package)))
        .collect();
    let mut found = BTreeSet::new();
    let mut pending = vec![package];
    while let Some(name) = pending.pop() {
        let owner = packages
            .get(name)
            .unwrap_or_else(|| panic!("{name} is not a member of the metadata workspace"));
        for dependency in &owner.dependencies {
            let path = dependency.path.as_deref();
            let Some(directory) = path.and_then(|path| path.file_name()) else {
                continue;
            };
            let copied = directory != STAND_IN && directory != package;
            if copied && found.insert(directory.to_owned()) {
                pending.push(directory);
            }
        }
    }
    found.into_iter().collect()
}

/// The dependencies of `kinds` a manifest declares, including its target-specific tables.
pub fn dependency_entries<'a>(
    manifest: &'a toml::Table,
    kinds: &'a [&str],
) -> impl Iterator<Item = (&'a String, &'a toml::Value)> {
    let targets = manifest
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table);
    std::iter::once(manifest)
        .chain(targets)
        .flat_map(move |table| kinds.iter().filter_map(|kind| table.get(*kind)))
        .filter_map(toml::Value::as_table)
        .flat_map(toml::Table::iter)
}

/// Write the manifest and empty library of local crate `name` below `root`.
fn write_crate(root: &Path, name: &str, dependencies: &str) {
    let directory = root.join("crates").join(name);
    fs::create_dir_all(directory.join("src")).unwrap();
    fs::write(directory.join("src/lib.rs"), "").unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        format!("[package]\nname = '{name}'\nversion = '0.0.0'\n{dependencies}"),
    )
    .unwrap();
}

/// Direct, inherited, target-specific and transitive local dependencies are found along with a
/// dependency's own development dependency; the stand-in, the package itself and registry crates
/// are not.
#[test]
fn closure_follows_metadata_the_way_cargo_reads_it() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = ['crates/*']\n\n[workspace.dependencies]\n\
         inherited = { path = 'crates/inherited' }\n\
         maestro-dependencies = { path = 'crates/maestro-dependencies' }\n",
    )
    .unwrap();
    write_crate(
        root,
        "maestro-tooling",
        "[dependencies]\ndirect = { path = '../direct' }\nregistry = '=1.0.0'\n\
         maestro-dependencies.workspace = true\n\
         [target.'cfg(unix)'.dev-dependencies]\ninherited.workspace = true\n\
         [target.'cfg(windows)'.build-dependencies]\nbuilt = { path = '../built' }\n",
    );
    write_crate(
        root,
        "direct",
        "[dependencies]\ntransitive = { path = '../transitive' }\n\
         [dev-dependencies]\nunread = { path = '../unread' }\n\
         maestro-tooling = { path = '../maestro-tooling' }\n",
    );
    for name in [
        "inherited",
        "built",
        "transitive",
        "unread",
        "maestro-dependencies",
    ] {
        write_crate(root, name, "");
    }
    // The fixture command keeps the scratch workspace on a private cargo home.
    let output = toolchain_support::fixture_command(env!("CARGO"), root, None)
        .unwrap()
        .args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        local_dependencies(
            &serde_json::from_slice(&output.stdout).unwrap(),
            "maestro-tooling"
        ),
        ["built", "direct", "inherited", "transitive", "unread"]
    );
}
