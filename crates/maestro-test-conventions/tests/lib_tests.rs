//! Library test-target agreement through the public workspace check.
#![cfg(test)]

use std::path::PathBuf;

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// The one crate the fixtures build.
const OWNER: &str = "maestro-path";

/// A library whose tests live beside the code that they exercise.
const TESTED: &str = "//! Fixture.\n\npub fn add(left: u32, right: u32) -> u32 {\n    left + right\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds() {\n        assert_eq!(super::add(1, 2), 3);\n    }\n}\n";

/// A source that declares a test function without a test module.
const DECLARED: &str = "//! Fixture.\n\n#[test]\nfn adds() {\n    assert_eq!(1 + 2, 3);\n}\n";

/// A workspace holding the owner crate alone.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&[OWNER]);
    workspace
}

/// The owner's manifest.
fn manifest(workspace: &Workspace) -> PathBuf {
    workspace
        .root
        .path()
        .join("crates")
        .join(OWNER)
        .join("Cargo.toml")
}

/// Set `test = false` on the library target.
fn disable(workspace: &Workspace) {
    workspace.in_source_tests(OWNER, false);
}

/// Drop the library's test setting again.
fn enable(workspace: &Workspace) {
    workspace.in_source_tests(OWNER, true);
}

/// The diagnostic that names the owner's manifest.
fn rejected(workspace: &Workspace) -> String {
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains(&manifest(workspace).display().to_string()),
        "{error}"
    );
    error
}

#[test]
fn in_source_tests_need_a_runnable_library_target() {
    let workspace = workspace();
    source(&workspace, OWNER, "src/lib.rs", TESTED);
    disable(&workspace);
    rejected(&workspace);
    enable(&workspace);
    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_library_without_in_source_tests_needs_the_flag() {
    let workspace = workspace();
    enable(&workspace);
    rejected(&workspace);
    disable(&workspace);
    check_workspace(workspace.root.path()).unwrap();
}

#[test]
fn a_declared_test_counts_as_an_in_source_test() {
    let workspace = workspace();
    source(&workspace, OWNER, "src/extra.rs", DECLARED);
    check_workspace(workspace.root.path()).unwrap();
    disable(&workspace);
    rejected(&workspace);
}
