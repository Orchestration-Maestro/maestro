//! Real paths of native files.
//!
//! [`canonicalize`] resolves a native path through the file system: links are
//! followed and `.` and `..` resolved, so every spelling of one existing file
//! gives one path. On Windows the system reports that path in the verbatim form
//! (`\\?\D:\x`, `\\?\UNC\server\share\x`), which users, settings and external
//! programs do not write; this crate returns the ordinary form (`D:\x`,
//! `\\server\share\x`) instead. Comparisons, stored identities and paths handed
//! to another program then all see the spelling those programs use. Other
//! platforms return the native real path as it is.

use std::{
    io,
    path::{Path, PathBuf},
};

/// Resolve `path` to its real path, in the ordinary form on Windows.
///
/// A Windows real path that is not Unicode text keeps the form the system
/// reports, because the ordinary spelling is computed on text.
///
/// # Errors
/// Returns the file-system error that resolving `path` reports, such as
/// [`io::ErrorKind::NotFound`] for a missing entry.
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let real = std::fs::canonicalize(path)?;
    #[cfg(windows)]
    {
        if let Some(text) = real.to_str() {
            return Ok(PathBuf::from(
                maestro_path::win32::ordinary(text).into_owned(),
            ));
        }
    }
    Ok(real)
}
