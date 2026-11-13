//! Path work that reads the file system.
//!
//! The rest of this crate works on path text; this module reads the file system
//! the path names. Directory listing and real-path resolution exist on native
//! targets only, because a browser build has no file system to read.
//!
//! File systems return a directory's entries in their own order: hash order on
//! many Unix file systems, case-insensitive name order on NTFS. [`read_dir`]
//! gives its callers the same name order on every platform.
//!
//! [`canonicalize`] resolves a native path through the file system: links are
//! followed and `.` and `..` resolved, so every spelling of one existing file
//! gives one path. On Windows the system reports that path in the verbatim form
//! (`\\?\D:\x`, `\\?\UNC\server\share\x`), which users, settings and external
//! programs do not write; this module returns the ordinary form (`D:\x`,
//! `\\server\share\x`) instead. Comparisons, stored identities and paths handed
//! to another program then all see the spelling those programs use. Other
//! platforms return the native real path as it is.

#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::DirEntry,
    io,
    path::{Path, PathBuf},
};

/// The entries of the directory at `path`, sorted by file name.
///
/// Names compare by their platform encoding: bytes on Unix, code points on
/// Windows. Entries of every kind are kept; `.` and `..` are never listed.
///
/// # Errors
/// Returns the error opening the directory or reading any of its entries.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    let mut entries = std::fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_cached_key(DirEntry::file_name);
    Ok(entries)
}

/// Resolve `path` to its real path, in the ordinary form on Windows.
///
/// A Windows real path that is not Unicode text keeps the form the system
/// reports, because the ordinary spelling is computed on text.
///
/// # Errors
/// Returns the file-system error that resolving `path` reports, such as
/// [`io::ErrorKind::NotFound`] for a missing entry.
#[cfg(not(target_arch = "wasm32"))]
pub fn canonicalize(path: impl AsRef<Path>) -> io::Result<PathBuf> {
    let real = std::fs::canonicalize(path)?;
    #[cfg(windows)]
    {
        if let Some(text) = real.to_str() {
            return Ok(PathBuf::from(crate::win32::ordinary(text).into_owned()));
        }
    }
    Ok(real)
}
