//! Isolated filesystem entries for native socket tests.
#![cfg(test)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Distinguish test directories within one process.
static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Own a newly created test directory until its socket handles have dropped.
pub struct Directory(pub PathBuf);

impl Directory {
    /// Create a short unique directory without replacing an existing entry.
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "mipc-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
