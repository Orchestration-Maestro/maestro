//! Temporary directories, isolated child test processes and stored credential fixtures
//! shared by the storage tests.
#![allow(dead_code)] // each test binary uses a different part of these helpers
use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthStorageBackend, InMemoryAuthStorageBackend,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock, RwLockReadGuard};

/// Held shared by tests whose file locks must be released exactly when dropped, and
/// exclusively while a child process is created: a forked child owns a copy of every
/// open descriptor until it executes, so a lock released meanwhile would stay held.
pub(crate) static PROCESS_CREATION: RwLock<()> = RwLock::new(());

/// Serializes cache observations; acquire before `PROCESS_CREATION` when both are needed.
pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

/// Distinguishes fixture directories created in the same process.
static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

/// Disposable directory removed when dropped.
pub struct TempDir(PathBuf, Option<RwLockReadGuard<'static, ()>>);

impl TempDir {
    /// Create an empty directory named after the test.
    pub fn new(name: &str) -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "maestro-auth-{name}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path, None)
    }

    /// Like [`TempDir::new`], and keeps [`run_child`] from creating a process while it lives;
    /// never use it in a test that calls [`run_child`].
    pub fn without_forks(name: &str) -> Self {
        let guard = PROCESS_CREATION
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut dir = Self::new(name);
        dir.1 = Some(guard);
        dir
    }

    /// The directory itself.
    pub fn root(&self) -> &Path {
        &self.0
    }

    /// A path below the directory, as the text the storage API accepts.
    pub fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_str().unwrap().to_owned()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether this process is a child started by [`run_child`].
pub fn is_child() -> bool {
    std::env::var_os("MAESTRO_AUTH_CHILD").is_some()
}

/// Rerun one test in this binary with only `envs` set and `cwd` as working directory.
pub fn run_child(test: &str, cwd: &Path, envs: &[(&str, &str)]) {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture"])
        .env_clear()
        .env("MAESTRO_AUTH_CHILD", "1")
        .envs(envs.iter().copied())
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let child = {
        let _creation = PROCESS_CREATION
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        command.spawn().unwrap()
    };
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Stored API key credential.
pub fn api_key(key: &str) -> AuthCredential {
    AuthCredential::ApiKey(ApiKeyCredential {
        key: key.to_owned(),
    })
}

/// Text held by a memory backend.
pub fn memory_text(backend: &InMemoryAuthStorageBackend) -> Option<String> {
    let mut text = None;
    backend
        .with_lock(&mut |current| {
            text = current.map(str::to_owned);
            Ok(None)
        })
        .unwrap();
    text
}

/// Run `future` to completion on a fresh current-thread runtime.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// Run `future` on a current-thread runtime whose clock advances only when idle.
pub fn block_on_paused<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(future)
}

#[cfg(test)]
mod tests {
    use super::TempDir;

    #[test]
    fn same_label_directories_preserve_each_others_files() {
        let first = TempDir::new("repeated-label");
        std::fs::write(first.path("auth.json"), b"first credential").unwrap();
        let second = TempDir::new("repeated-label");
        assert_eq!(
            std::fs::read(first.path("auth.json")).unwrap(),
            b"first credential"
        );
        assert_ne!(first.root(), second.root());
        drop(second);
        assert!(first.root().is_dir());
    }
}
