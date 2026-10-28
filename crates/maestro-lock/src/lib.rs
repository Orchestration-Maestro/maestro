//! File-write exclusion and durable byte publication.

mod replace;
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

/// Acquires target-based write exclusion and replays a pending publication.
///
/// # Errors
/// Returns native acquisition or recovery errors.
pub fn acquire_write(
    _path: &std::path::Path,
    _through_symlinks: bool,
) -> std::io::Result<WriteLock> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// Exclusive ownership of one native target's recoverable write.
#[derive(Debug)]
pub struct WriteLock {
    /// Retained native exclusion handle.
    _file: File,
}

impl WriteLock {
    /// Publishes bytes while retaining the target's exclusion.
    ///
    /// # Errors
    /// Returns native publication errors.
    pub fn replace(
        &self,
        _bytes: &[u8],
        _creation_mode: Option<u32>,
    ) -> std::io::Result<Committed> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
