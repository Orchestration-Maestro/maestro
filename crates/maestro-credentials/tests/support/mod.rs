//! Temporary directories, isolated child test processes and stored credential fixtures
//! shared by the storage tests.
use maestro_credentials::{
    ApiKeyCredential, AuthCredential, AuthStorageBackend, InMemoryAuthStorageBackend,
};
use std::path::Path;
use std::sync::{Mutex, RwLock, RwLockReadGuard};

mod runtime;
pub use runtime::block_on;

/// Held shared by tests whose file locks must be released exactly when dropped, and
/// exclusively while a child process is created: a forked child owns a copy of every
/// open descriptor until it executes, so a lock released meanwhile would stay held.
pub(crate) static PROCESS_CREATION: RwLock<()> = RwLock::new(());

/// Serializes cache observations; acquire before `PROCESS_CREATION` when both are needed.
pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

/// Disposable directory removed when dropped.
pub struct TempDir(
    maestro_test_directory::Scratch,
    Option<RwLockReadGuard<'static, ()>>,
);

impl TempDir {
    /// Create an empty directory with the supplied creation-error context.
    pub fn new(name: &str) -> Self {
        Self(
            maestro_test_directory::Scratch::new()
                .unwrap_or_else(|error| panic!("create {name} fixture: {error}")),
            None,
        )
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
        self.0.path()
    }

    /// A path below the directory, as the text the storage API accepts.
    pub fn path(&self, relative: &str) -> String {
        self.0.path().join(relative).to_str().unwrap().to_owned()
    }
}

/// Whether this process is a child started by [`run_child`].
pub fn is_child() -> bool {
    maestro_test_process::selection().is_child()
}

/// Run the configured test process with supplied edits and cwd.
pub fn run_child(process: maestro_test_process::TestProcess, cwd: &Path, envs: &[(&str, &str)]) {
    let command = process
        .nocapture()
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
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Run the named test again in a clean child with only the supplied environment.
pub fn run_isolated(test: &str, envs: &[(&str, &str)]) {
    let dir = TempDir::new(test);
    let process = maestro_test_process::TestProcess::named(test).unwrap();
    run_child(process.env_clear().case("*"), dir.root(), envs);
}

/// Environment name carrying the selected case into a child test.
pub const CASE_ENV: &str = "MAESTRO_CASE";

/// Stored API key credential.
pub fn api_key(key: &str) -> AuthCredential {
    AuthCredential::ApiKey(ApiKeyCredential {
        env: None,
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

/// Run `future` on a current-thread runtime whose clock starts paused.
pub fn block_on_paused<F: std::future::Future>(future: F) -> F::Output {
    runtime::runtime(true).block_on(future)
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
