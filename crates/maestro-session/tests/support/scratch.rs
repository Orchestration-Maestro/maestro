//! Disposable directory removed on drop.
use std::path::PathBuf;

/// Scratch directory removed on drop.
pub struct Scratch(PathBuf);
impl Scratch {
    /// Create an empty owned directory.
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("maestro-tree-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    /// Directory spelling.
    pub fn text(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
