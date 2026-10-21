//! Real Git fixtures for repository discovery.
use std::path::Path;

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
