//! Shared controlled and native catalog files.
use maestro_catalog::{ConfigValues, ModelFileOperations, ModelRegistry};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::SharedHandle;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
/// Mutable file text for reload scenarios.
#[derive(Clone)]
pub struct Text(pub Arc<Mutex<Option<String>>>);
impl ModelFileOperations for Text {
    fn exists(&self, _: &str) -> bool {
        self.0.lock().unwrap().is_some()
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        self.0
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    }
}
/// Empty stored credentials.
#[must_use]
pub fn auth() -> SharedHandle<AuthStorage> {
    SharedHandle::new(AuthStorage::in_memory(AuthStorageData::new()))
}
/// Credential commands that answer no value and spawn no process.
pub struct NoCommands;

impl maestro_credentials::ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Credential commands that fail the test which requests one.
pub struct ForbiddenCommands;

impl maestro_credentials::ConfigValueOperations for ForbiddenCommands {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        panic!("a controlled catalog test requested the configured command {command:?}");
    }
}

/// Credential commands that fail every key command with no output.
#[must_use]
pub fn values() -> ConfigValues {
    SharedHandle::new(NoCommands)
}

/// Credential commands that must stay unrequested.
#[must_use]
pub fn forbidden() -> ConfigValues {
    SharedHandle::new(ForbiddenCommands)
}
/// Offline registry that resolves configured key commands through `values`.
#[must_use]
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn memory_with(values: ConfigValues) -> ModelRegistry {
    ModelRegistry::in_memory(auth(), values).unwrap()
}
/// Registry over controlled text.
#[must_use]
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn registry(text: &str) -> ModelRegistry {
    registry_with(text, values())
}
/// Registry over supplied controlled text that runs configured commands through `values`.
#[must_use]
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn text_registry(spelling: &str, text: &Text, values: ConfigValues) -> ModelRegistry {
    ModelRegistry::with_operations(auth(), spelling, text.clone(), values).unwrap()
}
/// Registry over controlled text that runs configured commands through `values`.
#[must_use]
/// # Panics
/// Panics when a controlled test operation or assertion fails.
pub fn registry_with(text: &str, values: ConfigValues) -> ModelRegistry {
    text_registry(
        "<models-file>",
        &Text(Arc::new(Mutex::new(Some(text.into())))),
        values,
    )
}
/// Disposable native file, created exclusively.
pub struct File(pub PathBuf);
impl File {
    #[must_use]
    /// # Panics
    /// Panics when a controlled test operation or assertion fails.
    pub fn new(bytes: &[u8]) -> Self {
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-catalog-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        Self(path)
    }
    #[must_use]
    /// # Panics
    /// Panics if the native temporary path is not Unicode.
    pub fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
