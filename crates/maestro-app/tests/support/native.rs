//! Controlled native command helpers.
use std::{path::Path, process::Command};

#[path = "child.rs"]
mod child;
pub use child::{Scratch, probe};

#[cfg(unix)]
/// Run `git` with fixed identity and no user configuration.
pub fn git(dir: &str, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("git starts");
    assert!(output.status.success(), "git {args:?} failed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[cfg(unix)]
/// Copy `source` to `target` as an executable through a child process.
pub fn install_executable(source: &Path, target: &Path) {
    let status = Command::new("install")
        .args(["-m", "0700"])
        .arg(source)
        .arg(target)
        .status()
        .expect("install starts");
    assert!(status.success(), "install failed");
}
