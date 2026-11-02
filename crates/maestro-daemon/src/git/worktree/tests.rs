//! Git failure classification, launch sources and defensive recovery.
use super::*;
use maestro_test_directory::Scratch;
use std::{error::Error, path::Path};

/// A completed Git failure with supplied diagnostic streams.
fn failed_output(stderr: &str, stdout: &str) -> Output {
    let mut output = git_output(Path::new("."), [os("--maestro-invalid-option")]).unwrap();
    assert!(!output.status.success());
    output.stderr = stderr.as_bytes().to_vec();
    output.stdout = stdout.as_bytes().to_vec();
    output
}

#[test]
fn git_failure_classifies_removal_refusals() {
    for (message, dirty, missing) in [
        (
            "fatal: '/w/repo' contains modified or untracked files, use --force to delete it",
            true,
            false,
        ),
        (
            "fatal: '/w/repo' is a missing but already registered worktree",
            false,
            false,
        ),
        (
            "fatal: '/w/repo' contains a locked worktree, use --force only if you know why",
            false,
            false,
        ),
        ("fatal: '/w/repo' is not a working tree", false, true),
        ("fatal: '/w/repo' IS NOT A WORKTREE", false, true),
    ] {
        let error = failure("worktree remove", &failed_output(message, ""));

        assert_eq!(error.is_dirty_checkout(), dirty, "{message}");
        assert_eq!(error.is_not_a_working_tree(), missing, "{message}");
        assert!(error.source().is_none());
    }
}

#[test]
fn git_failure_prefers_stderr_then_stdout_then_exit_status() {
    let mut output = failed_output("  diagnostic from stderr\n", "diagnostic from stdout");
    assert_eq!(
        failure("worktree list", &output).message(),
        "diagnostic from stderr"
    );
    output.stderr.clear();
    assert_eq!(
        failure("worktree list", &output).message(),
        "diagnostic from stdout"
    );
    output.stdout.clear();
    let error = failure("worktree list", &output);
    assert!(error.message().contains(&output.status.to_string()));
    assert!(error.message().contains("worktree list"));
    assert!(!error.is_dirty_checkout());
    assert!(!error.is_not_a_working_tree());
}

#[test]
fn launch_failure_retains_process_source() {
    let error = capture(Path::new("\0"), &[]).unwrap_err();
    let source = error.source().unwrap();

    assert!(source.downcast_ref::<SpawnSyncError>().is_some());
    let io = source
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert_eq!(io.kind(), std::io::ErrorKind::InvalidInput);
    assert!(!error.is_dirty_checkout());
    assert!(!error.is_not_a_working_tree());
}

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
    let refusal = failure(
        "worktree remove",
        &failed_output("fatal: 'listed' is not a working tree", ""),
    );
    let message = refusal.message().to_owned();

    let error = recover_leftover(&repo, &checkout, refusal).unwrap_err();

    assert_eq!(error.message(), message);
    assert!(checkout.join(".git").exists());
}
