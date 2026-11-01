//! File-write exclusion and durable byte publication.

mod atomic;
mod identity;
mod names;
pub use atomic::{Symlinks, Temporary, atomic_replace};
mod recovery;
mod replace;
pub use identity::{acquire_write, acquire_write_bounded, try_acquire_write};
pub use replace::{Committed, Replace, replace};

use std::fs::{File, TryLockError};
use std::time::Duration;

/// Attempts acquisition with controlled attempt and wait effects.
fn retry_contended<T>(
    mut attempt: impl FnMut() -> Result<T, TryLockError>,
    mut wait: impl FnMut(Duration),
) -> Result<T, TryLockError> {
    for _ in 0..9 {
        match attempt() {
            Err(TryLockError::WouldBlock) => wait(Duration::from_millis(20)),
            result => return result,
        }
    }
    attempt()
}

#[cfg(test)]
mod tests;

/// Exclusive ownership of one canonical native target's recoverable write.
#[derive(Debug)]
pub struct WriteLock {
    /// Resolved canonical entry, leaving caller-authored paths untouched.
    pub(crate) target: std::path::PathBuf,
    /// Durable intended-byte record beside the target.
    pub(crate) recovery: std::path::PathBuf,
    /// Name removed while this holder still owns native exclusion.
    #[cfg(not(windows))]
    sidecar: std::path::PathBuf,
    /// Retained native exclusion handle; Windows deletes its name when sharing handles close.
    _file: File,
}

impl WriteLock {
    /// The canonical entry protected by this holder.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.target
    }

    /// Consumes this holder to append once, persisting recovery absence before writing.
    ///
    /// A missing target is created with native default permissions. The appended
    /// bytes are not synced; the caller retains its append durability policy.
    ///
    /// # Errors
    /// Returns native directory-sync, open or append errors before releasing exclusion.
    pub fn append(self, bytes: &[u8]) -> std::io::Result<()> {
        recovery::append(&self, bytes)
    }

    /// Consumes this holder to publish one write, retaining native identity and access controls.
    ///
    /// # Errors
    /// Returns native staging or target-write errors. A pending durable copy is
    /// replayed before the next holder is admitted.
    pub fn replace(self, bytes: &[u8], creation_mode: Option<u32>) -> std::io::Result<Committed> {
        recovery::replace(&self, bytes, creation_mode)
    }
}

#[cfg(not(windows))]
impl Drop for WriteLock {
    fn drop(&mut self) {
        // A refused removal leaves a reusable lock file; closing still releases exclusion.
        let _ = std::fs::remove_file(&self.sidecar);
    }
}
