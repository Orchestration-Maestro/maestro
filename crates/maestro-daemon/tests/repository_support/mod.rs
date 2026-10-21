//! Real Git fixtures for repository discovery.
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn temp_test_dir(name: &str) -> PathBuf {
    let unique = format!(
        "maestro-workspace-tests-{}-{}-{}",
        name,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = std::env::temp_dir().join(unique);
    std::fs::create_dir_all(&path).unwrap();
    path
}

pub fn init_repo_with_commit(repo: &Path) {
    std::fs::create_dir_all(repo).unwrap();
    run_git(repo, &["init", "--quiet"]);
    run_git(repo, &["config", "user.email", "maestro@example.invalid"]);
    run_git(repo, &["config", "user.name", "Maestro Test"]);
    run_git(
        repo,
        &["commit", "--quiet", "--allow-empty", "-m", "initial"],
    );
}

pub fn run_git(cwd: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}
