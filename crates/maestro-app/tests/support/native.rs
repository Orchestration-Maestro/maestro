//! Controlled native command helpers.
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::child;
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
/// Install the prepared native fixture as `name` below `directory`, selected by `record`.
pub fn command(directory: &Path, name: &str, record: &serde_json::Value) -> PathBuf {
    std::fs::create_dir_all(directory).expect("the alias directory is creatable");
    let path = directory.join(name);
    maestro_test_process::install_command(&path, &record.to_string()).expect("install starts");
    path
}

#[cfg(unix)]
/// Whether this process is refused access that a mode takes away; a privileged run is not.
pub fn denials() -> bool {
    let probe = maestro_test_directory::Scratch::new().unwrap();
    maestro_test_directory::deny(probe.path(), 0o0)
        .unwrap()
        .is_some()
}
