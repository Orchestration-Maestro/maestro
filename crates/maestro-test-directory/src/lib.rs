//! Exclusive disposable directory ownership for native tests.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Sequence shared by all callers in this process.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// An exclusively created directory whose removal is attempted on drop.
#[derive(Debug)]
pub struct Scratch {
    /// The spelling selected when this owner was created.
    path: PathBuf,
}

impl Scratch {
    /// Create a directory under the native temporary root and retain its canonical path.
    ///
    /// # Errors
    /// Returns a directory creation or canonicalization failure.
    pub fn new() -> io::Result<Self> {
        let mut scratch = Self::new_in(&std::env::temp_dir())?;
        scratch.path = fs::canonicalize(&scratch.path)?;
        Ok(scratch)
    }

    /// Create a directory under an existing parent using native path joining, without canonicalization.
    ///
    /// # Errors
    /// Returns a directory creation failure other than a candidate name collision.
    pub fn new_in(parent: &Path) -> io::Result<Self> {
        allocate(parent, &NEXT)
    }

    /// The owned directory, with the spelling selected at creation.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Retry only existing candidate names, retaining every pre-existing entry.
fn allocate(parent: &Path, sequence: &AtomicU64) -> io::Result<Scratch> {
    loop {
        let path = parent.join(format!(
            "maestro-test-{}-{}",
            std::process::id(),
            sequence.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(Scratch { path }),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests;
