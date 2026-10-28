//! Isolated filesystem entries for native socket tests.
#![cfg(test)]

use std::path::Path;

/// Own a newly created test directory until its socket handles have dropped.
pub struct Directory(maestro_test_directory::Scratch);

impl Directory {
    /// Create a unique directory without replacing an existing entry.
    pub fn new() -> Self {
        Self(maestro_test_directory::Scratch::new_socket().unwrap())
    }
    /// The directory this test owns.
    pub fn path(&self) -> &Path {
        self.0.path()
    }
}
