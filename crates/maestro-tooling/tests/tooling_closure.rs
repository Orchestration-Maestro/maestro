//! The workspace crates a tooling fixture must copy, read from the manifests.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The crate every fixture replaces with an empty stand-in, so its sources are never copied.
const STAND_IN: &str = "maestro-dependencies";

/// The dependency kinds cargo reads for the package it builds.
const OWN_KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The dependency kinds cargo reads for a dependency of the package it builds.
const DEPENDENCY_KINDS: [&str; 2] = ["dependencies", "build-dependencies"];

/// The local crates the tooling package needs beside itself.
pub fn tooling_local_dependencies() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    local_dependencies(&root, "maestro-tooling")
}

/// The local crates `package` needs beside itself, below the workspace `root`.
///
/// Cargo reads the manifest of each local dependency of the package, development ones
/// included, but none of a dependency's own development dependencies.
fn local_dependencies(root: &Path, package: &str) -> Vec<String> {
    let inherited = manifest(&root.join("Cargo.toml"))
        .remove("workspace")
        .and_then(|workspace| workspace.try_into::<toml::Table>().ok())
        .and_then(|mut workspace| workspace.remove("dependencies"))
        .and_then(|dependencies| dependencies.try_into::<toml::Table>().ok())
        .unwrap_or_default();
    let mut found = BTreeSet::new();
    let mut pending = vec![(package.to_owned(), OWN_KINDS.as_slice())];
    while let Some((name, kinds)) = pending.pop() {
        let table = manifest(&root.join("crates").join(&name).join("Cargo.toml"));
        for (dependency, entry) in dependency_entries(&table, kinds) {
            let entry = if entry.get("workspace").is_some() {
                inherited.get(dependency).unwrap_or(entry)
            } else {
                entry
            };
            let Some(directory) = entry
                .get("path")
                .and_then(toml::Value::as_str)
                .and_then(|path| Path::new(path).file_name())
                .and_then(|name| name.to_str())
            else {
                continue;
            };
            if directory != STAND_IN && directory != package && found.insert(directory.to_owned()) {
                pending.push((directory.to_owned(), DEPENDENCY_KINDS.as_slice()));
            }
        }
    }
    found.into_iter().collect()
}

/// Read one manifest.
fn manifest(path: &Path) -> toml::Table {
    fs::read_to_string(path).unwrap().parse().unwrap()
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

/// Write the manifest of local crate `name` below `root`.
fn write_crate(root: &Path, name: &str, dependencies: &str) {
    let directory = root.join("crates").join(name);
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        format!("[package]\nname = '{name}'\n{dependencies}"),
    )
    .unwrap();
}

/// Direct, inherited, target-specific and transitive local dependencies are found, and a
/// dependency's own development dependencies, the stand-in and registry crates are not.
#[test]
fn closure_follows_manifests_the_way_cargo_reads_them() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace.dependencies]\ninherited = { path = 'crates/inherited' }\n\
         maestro-dependencies = { path = 'crates/maestro-dependencies' }\n",
    )
    .unwrap();
    write_crate(
        root,
        "owner",
        "[dependencies]\ndirect = { path = '../direct' }\nregistry = '=1.0.0'\n\
         maestro-dependencies.workspace = true\n\
         [target.'cfg(unix)'.dev-dependencies]\ninherited.workspace = true\n\
         [target.'cfg(windows)'.build-dependencies]\nbuilt = { path = '../built' }\n",
    );
    write_crate(
        root,
        "direct",
        "[dependencies]\ntransitive = { path = '../transitive' }\n\
         [dev-dependencies]\nunread = { path = '../unread' }\n",
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
    assert_eq!(
        local_dependencies(root, "owner"),
        ["built", "direct", "inherited", "transitive"]
    );
}
