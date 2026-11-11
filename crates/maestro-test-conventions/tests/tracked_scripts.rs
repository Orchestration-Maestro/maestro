#![cfg(test)]

use crate::support::Workspace;
use maestro_test_conventions::check_workspace;
use std::path::Path;
use std::process::Command;

/// Run `git` in the fixture root and require success.
fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

/// Write a file under the fixture root; `tracked` also adds it to a fresh repository's index.
fn script(workspace: &Workspace, path: &str, tracked: bool) {
    let root = workspace.root.path();
    if !root.join(".git").exists() {
        git(root, &["init", "-q"]);
    }
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "x\n").unwrap();
    if tracked {
        git(root, &["add", "--", path]);
    }
}

#[test]
fn tracked_script_files_are_rejected_in_every_banned_language() {
    for name in [
        "a.js",
        "nested/b.mjs",
        "c.cjs",
        "d.jsx",
        "e.ts",
        "f.tsx",
        "docs/g.py",
        "h.sh",
        "i.ps1",
    ] {
        let workspace = Workspace::with_text_member();
        script(&workspace, name, true);

        let error = check_workspace(workspace.root.path()).unwrap_err();

        assert!(error.contains(name), "{error}");
        assert!(error.contains("Rust"), "{error}");
    }
}

#[test]
fn the_pending_example_is_the_only_tracked_script_accepted() {
    let workspace = Workspace::with_text_member();
    script(&workspace, "examples/sdk/rpc.py", true);
    check_workspace(workspace.root.path()).unwrap();

    script(&workspace, "examples/sdk/other.py", true);
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(error.contains("other.py"), "{error}");
}

#[test]
fn untracked_scripts_and_other_files_are_not_checked() {
    let workspace = Workspace::with_text_member();
    script(&workspace, "target/sandbox-runtime/loader.js", false);
    script(&workspace, "docs/notes.md", true);
    script(&workspace, "crates/text/assets/data.json", true);

    check_workspace(workspace.root.path()).unwrap();
}
