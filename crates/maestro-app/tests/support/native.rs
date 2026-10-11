//! Disposable filesystem and child-process probes.
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};
#[cfg(unix)]
/// A directory removed when dropped, on a best-effort basis.
pub struct Scratch(pub PathBuf);

#[cfg(unix)]
impl Scratch {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-footer-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("scratch directory is new");
        Self(path)
    }

    pub fn join(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }
}

#[cfg(unix)]
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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
/// Run this test binary again for the single test `name`, with `envs`, and
/// return the `PROBE ` lines the child printed.
pub fn probe(name: &str, current_dir: &Path, envs: &[(&str, &str)]) -> Vec<String> {
    let output = Command::new(std::env::current_exe().expect("test binary path"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .current_dir(current_dir)
        .envs(envs.iter().copied())
        .output()
        .expect("child starts");
    assert!(output.status.success(), "child probe failed");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("PROBE ").map(str::to_owned))
        .collect()
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
