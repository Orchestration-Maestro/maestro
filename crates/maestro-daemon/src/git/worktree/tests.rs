//! Recovery cases that the public operations cannot reach with a real repository.
use super::{WorktreeError, recover_leftover};
use maestro_test_directory::Scratch;
use std::path::Path;

/// Runs one Git command that must succeed.
fn git(cwd: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?} failed");
}

#[test]
fn recovery_keeps_a_checkout_git_still_lists() {
    let scratch = Scratch::new().unwrap();
    let repo = scratch.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["config", "user.email", "maestro@example.invalid"]);
    git(&repo, &["config", "user.name", "Maestro Test"]);
    git(
        &repo,
        &["commit", "--quiet", "--allow-empty", "-m", "initial"],
    );
    let checkout = scratch.path().join("trees/listed");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "listed",
            checkout.to_str().unwrap(),
        ],
    );
    let refusal = WorktreeError::new("fatal: 'listed' is not a working tree");

    let result = recover_leftover(&repo, &checkout, refusal.clone());

    assert_eq!(result, Err(refusal));
    assert!(checkout.join(".git").exists());
}
