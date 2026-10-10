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
    pub fn new() -> io::Result<Self> {
        let name = format!(
            "maestro-packages-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let path = std::env::temp_dir().join(name);
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}
