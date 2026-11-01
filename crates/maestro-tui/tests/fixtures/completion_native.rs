//! Owned temporary directories for native completion tests.

use std::path::Path;

/// A disposable directory owned by one test.
pub struct Tree(maestro_test_directory::Scratch);
impl Tree {
    /// Create an empty directory without modifying the runner environment.
    ///
    /// # Errors
    /// Returns a directory creation error.
    pub fn new() -> std::io::Result<Self> {
        maestro_test_directory::Scratch::new().map(Self)
    }
    /// The directory this test owns.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.0.path()
    }
    /// Authored spelling for provider construction.
    #[must_use]
    pub fn authored(&self) -> String {
        self.0.path().to_string_lossy().into_owned()
    }
    /// Create a regular file in the owned directory.
    ///
    /// # Errors
    /// Returns a file creation error.
    pub fn file(&self, name: &str) -> std::io::Result<()> {
        std::fs::write(self.0.path().join(name), b"")
    }
    /// Create a child directory.
    ///
    /// # Errors
    /// Returns a directory creation error.
    pub fn directory(&self, name: &str) -> std::io::Result<()> {
        std::fs::create_dir(self.0.path().join(name))
    }
    /// Whether this host keeps every authored path apart, asked before any is created. When
    /// it does not, the groups it folds are reported under `label`.
    ///
    /// # Panics
    /// Panics when the host cannot be probed.
    #[cfg(unix)]
    #[must_use]
    pub fn keeps_apart(&self, label: &str, paths: &[&str]) -> bool {
        let folded = self.0.folded_names(paths).expect("host probed");
        if !folded.is_empty() {
            eprintln!("{label} needs names this host keeps apart: {folded:?}");
        }
        folded.is_empty()
    }
}
