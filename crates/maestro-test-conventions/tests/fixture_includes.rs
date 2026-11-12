//! Fixture size limits for test code that compiles its input.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use std::path::{Path, PathBuf};
use support::{Workspace, source};

/// The limit the check applies, in bytes.
const LIMIT: usize = 1_000_000;

/// A workspace whose one test file includes a fixture of `size` bytes beside it; returns the
/// workspace and the test file's path.
fn workspace(size: usize) -> (Workspace, PathBuf) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-daemon"]);
    let path = source(
        &workspace,
        "maestro-daemon",
        "src/probe/tests.rs",
        "#[test]\nfn reads() {\n    let _ = include_str!(\"corpus.json\");\n}\n",
    );
    std::fs::write(path.with_file_name("corpus.json"), vec![b' '; size]).unwrap();
    (workspace, path)
}

/// The diagnostic for an over-limit include of `size` bytes at line 3 of `path`.
fn over(path: &Path, size: usize) -> String {
    format!(
        "{}:3: tests compile corpus.json ({size} bytes) into their executable, over the {LIMIT}-byte limit; read it when the test runs instead",
        path.display()
    )
}

#[test]
fn over_limit_fixture_include_is_rejected() {
    let (workspace, path) = workspace(LIMIT + 1);
    assert_eq!(
        check_workspace(workspace.root.path()),
        Err(over(&path, LIMIT + 1))
    );
}

#[test]
fn fixture_include_at_the_limit_is_accepted() {
    let (workspace, _) = workspace(LIMIT);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
