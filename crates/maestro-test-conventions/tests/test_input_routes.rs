//! Routing of the repository directories that tests read at run time.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use std::path::PathBuf;
use support::{Workspace, source};

/// A workspace with one member whose test file holds `contents`, and a CI configuration
/// whose `test_inputs` table is `routes`; returns the workspace and the test file's path.
fn workspace(contents: &str, routes: &str) -> (Workspace, PathBuf) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-daemon"]);
    let path = source(&workspace, "maestro-daemon", "src/probe/tests.rs", contents);
    let configuration = workspace.root.path().join(".github");
    std::fs::create_dir_all(&configuration).unwrap();
    std::fs::write(
        configuration.join("ci.toml"),
        format!("test_inputs = {{ {routes} }}\n"),
    )
    .unwrap();
    (workspace, path)
}

/// The diagnostic for a read of `prefix` at `line` of `path`.
fn unrouted(path: &std::path::Path, line: usize, prefix: &str) -> String {
    format!(
        "{}:{line}: tests read {prefix} at run time, but test_inputs in .github/ci.toml does not route it to maestro-daemon",
        path.display()
    )
}

/// A test that reaches the workspace root through the manifest directory.
const MANIFEST_READ: &str = "fn brand() -> &'static str {\n    concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../../assets/brand/brand.json\")\n}\n";

/// A test that finds the workspace root by walking up, then joins a repository directory.
const ANCESTOR_READ: &str = "fn guide() -> String {\n    let root = std::path::Path::new(env!(\"CARGO_MANIFEST_DIR\")).ancestors().nth(2).unwrap();\n    std::fs::read_to_string(root.join(\"docs/extensions.md\")).unwrap()\n}\n";

#[test]
fn unrouted_manifest_relative_read_is_named() {
    let (workspace, path) = workspace(MANIFEST_READ, "\"assets/\" = []");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 2, "assets/"))
    );
}

#[test]
fn unrouted_read_through_the_workspace_root_is_named() {
    let (workspace, path) = workspace(ANCESTOR_READ, "\"docs/\" = [\"maestro-text\"]");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 3, "docs/"))
    );
}

#[test]
fn read_of_a_directory_with_no_route_entry_is_named() {
    let (workspace, path) = workspace(ANCESTOR_READ, "\"fuzz/\" = []");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 3, "docs/"))
    );
}

#[test]
fn routed_reads_are_accepted() {
    for (contents, routes) in [
        (MANIFEST_READ, "\"assets/\" = [\"maestro-daemon\"]"),
        (
            ANCESTOR_READ,
            "\"docs/\" = [\"maestro-daemon\", \"maestro-text\"]",
        ),
    ] {
        let (workspace, _) = workspace(contents, routes);
        assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    }
}

#[test]
fn reads_that_are_not_run_time_repository_reads_are_accepted() {
    let contents = r#"#[path = "../../examples/shared.rs"]
mod shared;
const COMPILED: &str = include_str!("../../../../docs/guide.md");
fn scratch(directory: &std::path::Path) -> std::path::PathBuf {
    directory.join("docs/guide.md")
}
"#;
    let (workspace, _) = workspace(contents, "\"docs/\" = [], \"examples/\" = []");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn production_code_is_not_a_test_read() {
    let (workspace, _) = workspace("", "\"assets/\" = []");
    source(&workspace, "maestro-daemon", "src/lib.rs", MANIFEST_READ);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn unrouted_read_through_a_package_directory_resolver_is_named() {
    for (call, prefix) in [
        ("get_docs_path()", "docs/"),
        ("get_examples_path()", "examples/"),
        ("paths::get_docs_path(&package)", "docs/"),
    ] {
        let contents = format!("fn folder() -> String {{\n    {call}.unwrap()\n}}\n");
        let (workspace, path) = workspace(&contents, "\"fuzz/\" = []");
        assert_eq!(
            check_workspace(workspace.root.path()),
            Err(unrouted(&path, 2, prefix)),
            "{call}"
        );
    }
}

#[test]
fn package_directory_read_names_the_docs_and_examples_it_holds() {
    let contents = "fn package() -> String {\n    get_package_dir().unwrap()\n}\n";
    let (workspace, path) = workspace(contents, "\"fuzz/\" = []");

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{}\n{}",
            unrouted(&path, 2, "docs/"),
            unrouted(&path, 2, "examples/")
        ))
    );
}

#[test]
fn unrouted_resolver_read_inside_a_macro_is_named() {
    let contents =
        "fn check() {\n    assert!(std::path::Path::new(&get_docs_path().unwrap()).is_dir());\n}\n";
    let (workspace, path) = workspace(contents, "\"fuzz/\" = []");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 2, "docs/"))
    );
}

#[test]
fn routed_resolver_reads_are_accepted() {
    let contents = "fn folder() -> String {\n    get_docs_path().unwrap()\n}\n";
    let (workspace, _) = workspace(contents, "\"docs/\" = [\"maestro-daemon\"]");
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn resolver_names_outside_test_code_or_as_declarations_are_not_reads() {
    let (workspace, _) = workspace(
        "fn get_docs_path() -> String {\n    String::new()\n}\n",
        "\"fuzz/\" = []",
    );
    source(
        &workspace,
        "maestro-daemon",
        "src/lib.rs",
        "fn folder() -> String {\n    get_docs_path().unwrap()\n}\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}

#[test]
fn ancestor_walk_inside_a_macro_makes_a_later_join_a_read() {
    let contents = "fn guide() -> String {\n    let root = dbg!(std::path::Path::new(\"x\").ancestors().nth(2).unwrap());\n    root.join(\"docs/x\").display().to_string()\n}\n";
    let (workspace, path) = workspace(contents, "\"fuzz/\" = []");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 3, "docs/"))
    );
}

#[test]
fn join_inside_a_macro_is_a_read_once_the_file_climbs() {
    let contents = "fn guide(root: &std::path::Path) {\n    let _ = \"../../crates\";\n    assert!(root.join(\"docs/x\").exists());\n}\n";
    let (workspace, path) = workspace(contents, "\"fuzz/\" = []");
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unrouted(&path, 3, "docs/"))
    );
}
