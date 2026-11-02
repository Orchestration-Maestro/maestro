//! Git fixtures and local executors for native component examples.
#![cfg(test)]

use std::{path::Path, process::Command};

/// Runs Git only inside an exclusively owned disposable repository, with the environment
/// the example's own Git commands inherit.
pub fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Runs the calling test again in a child whose Git commands, the example's included, read no
/// system or global configuration; returns `true` in the parent once that child passed.
pub fn isolated(test: &str) -> bool {
    if maestro_test_process::selection().is_child() {
        return false;
    }
    let status = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .case("*")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .nocapture()
        .status()
        .unwrap();
    assert!(status.success(), "{test}: {status}");
    true
}

/// Runs native process futures on their caller-owned local executor.
pub fn run(action: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, action);
}
