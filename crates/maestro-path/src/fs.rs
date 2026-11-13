//! Path work that reads the file system or the process directories.
//!
//! The rest of this crate works on path text; this module reads what that text
//! leaves open. Resolution reads the process working directory and the Windows
//! drive directories, and builds for every target. Directory listing and
//! real-path resolution exist on native targets only, because a browser build
//! has no file system to read.

use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

/// The current directory of each Windows drive, keyed by uppercase letter: the `=X:`
/// environment variables of the drives that have one. Other platforms have none.
#[must_use]
pub fn drive_dirs() -> Vec<(char, String)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    crate::env_drive_directories(|name| std::env::var_os(name))
}

/// The process working directory.
///
/// # Errors
/// Returns the I/O cause when the directory cannot be read.
pub fn current_dir() -> io::Result<String> {
    Ok(std::env::current_dir()?.to_string_lossy().into_owned())
}

/// Resolve each operand group from right to left, reading the process working
/// directory and the drive directories only when an operand leaves a part open.
///
/// Every group sees one reading of those directories, and the working directory
/// reaches its text through [`current_dir`].
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve_groups(groups: &[&[&str]]) -> io::Result<Vec<String>> {
    resolve_groups_with(groups, current_dir)
}

/// Resolve each operand group, reading the working directory through
/// `current_directory`, which keeps the caller's own text conversion.
///
/// # Errors
/// Returns the error `current_directory` returned, unchanged, when a group
/// needs the working directory.
pub fn resolve_groups_with(
    groups: &[&[&str]],
    current_directory: impl FnOnce() -> io::Result<String>,
) -> io::Result<Vec<String>> {
    crate::resolve_with(groups, drive_dirs, current_directory)
}

/// Resolve `paths` from right to left, reading the process working directory and the
/// drive directories only when the operands leave a part open.
///
/// Each resolution reads them anew.
///
/// # Errors
/// Returns the working directory's error when it was needed and unreadable.
pub fn resolve(paths: &[&str]) -> io::Result<String> {
    resolve_groups(&[paths]).map(|resolved| resolved.into_iter().collect::<String>())
}

/// The entries of the directory at `path`, sorted by file name.
///
/// File systems return a directory's entries in their own order: hash order on
/// many Unix file systems, case-insensitive name order on NTFS. Callers get the
/// same name order on every platform.
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
/// Links are followed and `.` and `..` resolved, so every spelling of one
/// existing file gives one path. On Windows the system reports that path in the
/// verbatim form (`\\?\D:\x`, `\\?\UNC\server\share\x`), which users, settings
/// and external programs do not write; the result is the ordinary form (`D:\x`,
/// `\\server\share\x`) instead. Comparisons, stored identities and paths handed
/// to another program then all see the spelling those programs use. Other
/// platforms return the native real path as it is.
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
