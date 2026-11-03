//! Conditional native session publication.
use super::files::{Hashing, Snapshot};
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
    rewrite_unless_changed(path, expected, bytes, read_native)
}

/// Rewrites a session like `rewrite_session_file`, returning the records now stored on mismatch.
pub(super) fn rewrite_snapshot(
    path: &str,
    expected: &[u8; 32],
    bytes: &[u8],
) -> io::Result<Option<Snapshot>> {
    rewrite_unless_changed(path, expected, bytes, snapshot_native)
}

/// Replaces the locked target when its digest is `expected`, otherwise observes it with `observe`.
fn rewrite_unless_changed<T>(
    path: &str,
    expected: &[u8; 32],
    bytes: &[u8],
    observe: impl FnOnce(&Path) -> io::Result<T>,
) -> io::Result<Option<T>> {
    let holder = maestro_lock::acquire_write(Path::new(path), true)?;
    if digest_native(holder.path())? != *expected {
        return observe(holder.path()).map(Some);
    }
    #[cfg(test)]
    tests::compared();
    holder.replace(bytes, None)?;
    Ok(None)
}

/// Runs a read or append under exclusion when the directory accepts the lock file.
pub(super) fn with_write_lock<T>(
    path: &str,
    operation: impl FnOnce(Option<maestro_lock::WriteLock>) -> io::Result<T>,
) -> io::Result<T> {
    match maestro_lock::acquire_write(Path::new(path), true) {
        Ok(holder) => operation(Some(holder)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::PermissionDenied
                    | io::ErrorKind::ReadOnlyFilesystem
            ) =>
        {
            operation(None)
        }
        Err(error) => Err(error),
    }
}

/// Replays and reads native history under exclusion before a writable owner loads it.
pub(super) fn read(path: &str) -> io::Result<Snapshot> {
    with_write_lock(path, |holder| match holder {
        Some(holder) => snapshot_native(holder.path()),
        None => snapshot_native(Path::new(path)),
    })
}

/// Reads the records of the selected target line by line; an absent target is empty.
pub(super) fn snapshot_native(path: &Path) -> io::Result<Snapshot> {
    match fs::File::open(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Snapshot::absent()),
        result => Snapshot::read(result?),
    }
}

/// Hashes the selected target without keeping its bytes; an absent target hashes as empty.
fn digest_native(path: &Path) -> io::Result<[u8; 32]> {
    let mut hashing = match fs::File::open(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Snapshot::absent().digest);
        }
        result => Hashing::new(result?),
    };
    io::copy(&mut hashing, &mut io::sink())?;
    Ok(hashing.finish().0)
}

/// Reads selected target bytes without changing their authored path representation.
pub(super) fn read_native(path: &Path) -> io::Result<Vec<u8>> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        result => result,
    }
}

#[cfg(test)]
mod tests;
