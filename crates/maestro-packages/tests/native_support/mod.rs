//! Owned native directories shared by package behavior tests.
#![cfg(test)]
use std::{io, path::PathBuf};
/// One exclusively-created disposable directory, removed on drop.
pub struct Scratch(pub PathBuf);
impl Scratch {
    /// Creates a fresh directory with an unused name.
    pub fn new() -> io::Result<Self> {
        let name = format!(
            "maestro-acquire-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
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

/// Creates a file fixture and its parent directory.
pub fn write(path: &str, text: &str) {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
