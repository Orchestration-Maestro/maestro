//! File-write exclusion and durable byte publication.

mod identity;
mod recovery;
mod replace;
pub use identity::acquire_write;
pub use replace::{Committed, Replace, replace};

use std::fs::{File, TryLockError};
use std::time::Duration;

/// Returns the supplied file after acquiring an exclusive lock.
///
/// Makes at most ten attempts, waiting 20 ms only between contended attempts.
///
/// # Errors
/// Returns the final contention error or the first non-contention error unchanged.
pub fn acquire(file: File) -> Result<File, TryLockError> {
    retry_contended(|| file.try_lock(), std::thread::sleep)?;
    Ok(file)
}

/// Returns the supplied file after waiting for exclusive ownership.
///
/// # Errors
/// Propagates native lock acquisition errors.
pub fn acquire_waiting(file: File) -> std::io::Result<File> {
    file.lock()?;
    Ok(file)
}

/// Attempts acquisition with controlled attempt and wait effects.
fn retry_contended(
    mut attempt: impl FnMut() -> Result<(), TryLockError>,
    mut wait: impl FnMut(Duration),
) -> Result<(), TryLockError> {
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
    /// Durable intended-byte record in the shared per-user state directory.
    pub(crate) recovery: std::path::PathBuf,
    /// Target identity carried by the recovery record.
    pub(crate) digest: [u8; 32],
    /// Retained native exclusion handle.
    _file: File,
}

impl WriteLock {
    /// The canonical entry protected by this holder.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.target
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
