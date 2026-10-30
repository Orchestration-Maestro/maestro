//! Unit coverage for the preparation build: its environment filter and the inputs it fingerprints.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::Path;

use cargo_metadata::Metadata;

use super::{built_crates, closure_directories, inherited_components, input_names, listed};

/// Every name component discovery accepts is removed, including the bare and doubled
/// underscore forms, and no other name is.
#[test]
fn inherited_components_follow_the_discovery_rule() {
    let names = [
        "MAESTRO_AUTHOR_COMPONENT",
        "MAESTRO_COMPONENT",
        "MAESTRO__COMPONENT",
        "MAESTRO_HOME",
        "MAESTRO_lower_COMPONENT",
        "PATH",
    ]
    .map(OsString::from);
    assert_eq!(
        inherited_components(names.into_iter()),
        [
            "MAESTRO_AUTHOR_COMPONENT",
            "MAESTRO_COMPONENT",
            "MAESTRO__COMPONENT"
        ]
        .map(OsString::from)
    );
}

/// Whether a `test_prepare_inputs` pattern covers a file: the pattern names the file, or it is a
/// directory followed by `/**`, which covers everything below the directory at any depth.
fn covers(pattern: &str, name: &str) -> bool {
    pattern
        .strip_suffix("/**")
        .map_or(pattern == name, |directory| {
            name.strip_prefix(directory)
                .is_some_and(|rest| rest.starts_with('/'))
        })
}

/// Shared CI keys the cached bundle on the files these patterns match, so the patterns name
/// exactly the inputs the preparation fingerprints: a crate it builds from that no pattern
/// covers could change while a stale bundle is restored, and a pattern for anything else
/// rebuilds the bundle for no reason or fails the job when it matches nothing.
#[test]
fn ci_cache_key_names_exactly_the_files_the_preparation_reads() {
    let ci: toml::Table = include_str!("../../../../.github/ci.toml").parse().unwrap();
    let declared: BTreeSet<String> = ci["test_prepare_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pattern| pattern.as_str().unwrap().to_owned())
        .collect();
    for pattern in &declared {
        let literal = pattern.strip_suffix("/**").unwrap_or(pattern);
        assert!(
            !literal.contains(['*', '?', '[', '{']),
            "unsupported {pattern}"
        );
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let crates = built_crates(Path::new(env!("CARGO")), &root).unwrap();
    let read = listed(&root, &crates).unwrap();

    let derived: BTreeSet<String> = input_names(&crates)
        .filter_map(|name| {
            let directory = format!("{name}/**");
            if read.contains(name) {
                Some(name.to_owned())
            } else {
                read.iter()
                    .any(|file| covers(&directory, file))
                    .then_some(directory)
            }
        })
        .collect();

    let missing: Vec<&String> = derived.difference(&declared).collect();
    let extra: Vec<&String> = declared.difference(&derived).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "no pattern for {missing:?}; patterns outside the inputs: {extra:?}"
    );
}

/// The locked metadata of a workspace whose members `a` to `g` depend on each other.
const FIXTURE: &str = include_str!("../../tests/fixtures/selection-metadata.json");

/// The directories the closure of `built` names in the fixture workspace.
fn closure_of(built: &[&str]) -> Vec<String> {
    let metadata: Metadata = serde_json::from_str(FIXTURE).unwrap();
    closure_directories(&metadata, built)
        .unwrap()
        .into_iter()
        .collect()
}

/// The closure follows normal, development and build dependencies and cycles, and leaves out
/// the callers of a package and a dependency that sits outside the workspace.
#[test]
fn closure_directories_hold_the_built_packages_and_what_they_depend_on() {
    assert_eq!(
        closure_of(&["d"]),
        ["crates/a", "crates/b", "crates/c", "crates/d", "crates/f"]
    );
    assert_eq!(closure_of(&["external-reader"]), ["crates/external-reader"]);
    assert_eq!(
        closure_of(&["alone", "nested"]),
        ["crates/a/nested", "crates/alone"]
    );
}

/// A built package the workspace does not contain is an error, so a rename cannot empty the set.
#[test]
fn closure_directories_reject_a_missing_built_package() {
    let metadata: Metadata = serde_json::from_str(FIXTURE).unwrap();
    assert!(closure_directories(&metadata, &["missing"]).is_err());
}
