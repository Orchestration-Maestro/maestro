//! Conditional native session publication.
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};

/// Replaces a session only when its SHA-256 matches `expected`.
///
/// Returns the newly observed bytes on a mismatch so the caller can check again.
/// A missing session has the digest of empty bytes. Success returns `None`.
/// The stable sidecar stays locked from the final read through atomic publication.
/// A parent-directory sync failure after publication does not fail the write.
///
/// # Errors
/// Propagates native lock, read and replacement errors before publication.
pub fn rewrite_session_file(
    path: &str,
    expected: &[u8; 32],
    bytes: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    with_write_lock(path, || {
        let current = read(path)?;
        if Sha256::digest(&current).as_slice() != expected {
            return Ok(Some(current));
        }
        maestro_lock::replace(
            Path::new(path),
            bytes,
            maestro_lock::Replace {
                through_symlinks: true,
                ..maestro_lock::Replace::default()
            },
        )?;
        Ok(None)
    })
}

/// Runs exactly one file write while retaining its stable sidecar's exclusive lock.
pub(super) fn with_write_lock<T>(
    path: &str,
    write: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    let sidecar = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(format!("{path}.lock"))?;
    let _lock = maestro_lock::acquire_waiting(sidecar)?;
    write()
}

/// Reads native session bytes, treating an absent history as empty.
pub(super) fn read(path: &str) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        result => result,
    }
}
