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
