//! Order of the rows that every crate step edits.
#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;
use std::path::{Path, PathBuf};

/// A workspace holding `configuration` as its CI configuration and `specification` as its
/// documented crate graph.
fn workspace(configuration: &str, specification: &str) -> (Workspace, PathBuf, PathBuf) {
    let workspace = Workspace::new();
    workspace.member("text", "maestro-text", "");
    workspace.list(&[("maestro-text", "core")]);
    let configuration = page(&workspace, ".github/ci.toml", configuration);
    let specification = page(&workspace, "docs/specs/maestro-port.md", specification);
    (workspace, configuration, specification)
}

/// Write `contents` at `path` in the fixture, returning the written path.
fn page(workspace: &Workspace, path: &str, contents: &str) -> PathBuf {
    let full = workspace.root.path().join(path);
    std::fs::create_dir_all(full.parent().unwrap_or_else(|| Path::new(""))).unwrap();
    std::fs::write(&full, contents).unwrap();
    full
}

/// The crate table of the documented graph, in the order `rows` gives it.
fn crate_table(rows: &[&str]) -> String {
    format!(
        "| Crate | One job | Allowed internal dependencies | Layer |\n|---|---|---|---|\n{}\n",
        rows.join("\n")
    )
}

/// The order diagnostic for `key`'s entries, naming `first` before `second` at `line` of `path`.
fn unordered(path: &Path, line: usize, key: &str, first: &str, second: &str) -> String {
    format!(
        "{}:{line}: {key} entries are not in name order: `{first}` is listed before `{second}`, which sorts before it",
        path.display()
    )
}

#[test]
fn a_ci_array_out_of_name_order_is_reported() {
    let configuration = "wasm_crates = [\n    \"maestro-web\",\n    \"maestro-agent\",\n]\n";
    let (workspace, path, _) = workspace(
        configuration,
        &crate_table(&["| `maestro-text` | own text | — | below 0 |"]),
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unordered(
            &path,
            3,
            "wasm_crates",
            "maestro-web",
            "maestro-agent"
        ))
    );
}

#[test]
fn a_route_out_of_name_order_is_reported() {
    let configuration =
        "test_inputs.\"docs/\" = [\n    \"maestro-web\",\n    \"maestro-app\",\n]\n";
    let (workspace, path, _) = workspace(
        configuration,
        &crate_table(&["| `maestro-text` | own text | — | below 0 |"]),
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(unordered(
            &path,
            3,
            "test_inputs.\"docs/\"",
            "maestro-web",
            "maestro-app"
        ))
    );
}

#[test]
fn a_ci_entry_written_on_the_key_line_is_reported() {
    let configuration = "test_prepare_inputs = [\"Cargo.toml\", \"Cargo.lock\"]\n";
    let (workspace, path, _) = workspace(
        configuration,
        &crate_table(&["| `maestro-text` | own text | — | below 0 |"]),
    );

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{}:1: test_prepare_inputs opens its entries on the key line; write one entry per line",
            path.display()
        ))
    );
}

#[test]
fn a_crate_table_row_out_of_name_order_is_reported() {
    let table = crate_table(&[
        "| `maestro-text` | own text | — | below 0 |",
        "| `maestro-app` | coordinate | `maestro-text` | 4 |",
    ]);
    let (workspace, _, path) = workspace("", &table);

    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(format!(
            "{}:4: crate table rows are not in crate-name order: `maestro-text` is listed before `maestro-app`, which sorts before it",
            path.display()
        ))
    );
}

#[test]
fn rows_in_name_order_are_accepted() {
    let configuration = "wasm_crates = [\n    \"maestro-agent\",\n    \"maestro-web\",\n]\ntest_prepare_inputs = [\n    \"Cargo.lock\",\n    \"assets/**\",\n]\ntest_inputs.\"docs/\" = []\n";
    let table = crate_table(&[
        "| `maestro-app` | coordinate | `maestro-text` | 4 |",
        "| `maestro-text` | own text | — | below 0 |",
    ]);
    let (workspace, _, _) = workspace(configuration, &table);

    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
