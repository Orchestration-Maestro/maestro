//! Disposable native directories shared by package behavior tests.
#![cfg(test)]
use std::{
    io,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
/// One exclusively-created disposable directory, removed on drop.
pub struct Scratch(pub PathBuf);
/// Distinguishes directories created by parallel tests in one process.
static NEXT: AtomicUsize = AtomicUsize::new(0);
impl Scratch {
    /// Creates a fresh directory whose name is unique within the process.
    /// # Errors
    /// Returns the directory creation failure.
    pub fn new() -> io::Result<Self> {
        Self::new_in(&std::env::temp_dir())
    }
    /// Creates a fresh directory on the supplied parent's drive.
    /// # Errors
    /// Returns the directory creation failure.
    pub fn new_in(parent: &std::path::Path) -> io::Result<Self> {
        let name = format!(
            "maestro-packages-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = parent.join(name);
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}
