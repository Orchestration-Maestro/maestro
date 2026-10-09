//! Owned temporary directories for native completion tests.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// Unique local directory counter.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// A disposable directory owned by one test.
pub struct Tree(
    /// The directory owned by this test.
    pub PathBuf,
);
impl Tree {
    /// Create an empty directory without modifying the runner environment.
    ///
    /// # Errors
    /// Returns a directory creation error.
    pub fn new() -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!(
            "maestro-completion-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
    /// Authored spelling for provider construction.
    #[must_use]
    pub fn authored(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
    /// Create a regular file in the owned directory.
    ///
    /// # Errors
    /// Returns a file creation error.
    pub fn file(&self, name: &str) -> std::io::Result<()> {
        std::fs::write(self.0.join(name), b"")
    }
    /// Create a child directory.
    ///
    /// # Errors
    /// Returns a directory creation error.
    pub fn directory(&self, name: &str) -> std::io::Result<()> {
        std::fs::create_dir(self.0.join(name))
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        assert!(std::fs::remove_dir_all(&self.0).is_ok());
    }
}
