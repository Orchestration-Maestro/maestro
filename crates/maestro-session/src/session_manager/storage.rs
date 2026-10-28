//! Conditional native session publication.
use sha2::{Digest, Sha256};
use std::{fs, io, path::Path};

/// Rewrites a session only when its locked SHA-256 matches `expected`.
///
/// Returns newly observed bytes on mismatch, or `None` after publication. An
/// absent session has the digest of empty bytes. Canonical target exclusion spans
/// recovery, the final read and comparison, and one recoverable in-place write.
/// Existing targets retain ownership, access controls and hard links. Authored
/// manager paths are unchanged. Postcommit durability warnings do not fail it.
///
/// # Errors
/// Propagates native lock, recovery, read and publication errors.
pub fn rewrite_session_file(
    path: &str,
    expected: &[u8; 32],
    bytes: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    let holder = maestro_lock::acquire_write(Path::new(path), true)?;
    let current = read_native(holder.path())?;
    if Sha256::digest(&current).as_slice() != expected {
        return Ok(Some(current));
    }
    #[cfg(test)]
    tests::compared();
    holder.replace(bytes, None)?;
    Ok(None)
}

/// Runs one file write under the canonical target's shared exclusion after recovery.
pub(super) fn with_write_lock<T>(
    path: &str,
    write: impl FnOnce(&Path) -> io::Result<T>,
) -> io::Result<T> {
    let holder = maestro_lock::acquire_write(Path::new(path), true)?;
    write(holder.path())
}

/// Replays and reads native history under exclusion before a writable owner loads it.
pub(super) fn read(path: &str) -> io::Result<Vec<u8>> {
    with_write_lock(path, read_native)
}

/// Reads selected target bytes without changing their authored path representation.
fn read_native(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        result => result,
    }
}

#[cfg(test)]
mod tests;
