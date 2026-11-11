#![cfg(test)]

use crate::support::Workspace;
use crate::tracked_scripts::git;
use maestro_test_conventions::check_workspace;

/// The largest tracked file the check accepts, in bytes.
const LIMIT: u64 = 10 * 1024 * 1024;

/// Write a sparse file of `length` bytes under the fixture root; `tracked` adds it to a fresh index.
fn sized(workspace: &Workspace, path: &str, length: u64, tracked: bool) {
    let root = workspace.root.path();
    if !root.join(".git").exists() {
        git(root, &["init", "-q"]);
    }
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::File::create(&file)
        .unwrap()
        .set_len(length)
        .unwrap();
    if tracked {
        git(root, &["add", "--", path]);
    }
}

#[test]
fn a_tracked_file_over_the_size_limit_is_rejected_by_path() {
    let workspace = Workspace::with_text_member();
    sized(
        &workspace,
        "crates/text/tests/fixtures/rows.json",
        LIMIT,
        true,
    );
    check_workspace(workspace.root.path()).unwrap();

    sized(
        &workspace,
        "crates/text/tests/fixtures/rows.json",
        LIMIT + 1,
        true,
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();

    assert!(
        error.contains("crates/text/tests/fixtures/rows.json"),
        "{error}"
    );
    assert!(error.contains("10485760"), "{error}");
}

#[test]
fn an_untracked_large_file_is_not_checked() {
    let workspace = Workspace::with_text_member();
    sized(&workspace, "target/archive.bin", LIMIT + 1, false);

    check_workspace(workspace.root.path()).unwrap();
}
