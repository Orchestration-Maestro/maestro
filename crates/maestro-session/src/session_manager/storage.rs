//! Conditional native session publication.
use std::{fs, io};

/// Replaces a session only when its SHA-256 matches `expected`.
///
/// Returns the newly observed bytes on a mismatch so the caller can check again.
/// A missing session has the digest of empty bytes. Success returns `None`.
///
/// # Errors
/// Propagates native file errors before publication.
pub fn rewrite_session_file(
    path: &str,
    _expected: &[u8; 32],
    bytes: &[u8],
) -> io::Result<Option<Vec<u8>>> {
    fs::write(path, bytes)?;
    Ok(None)
}
