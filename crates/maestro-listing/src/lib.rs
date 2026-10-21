//! Directory listing in name order.
//!
//! File systems return a directory's entries in their own order: hash order on
//! many Unix file systems, case-insensitive name order on NTFS. Every reader
//! that lists a directory goes through [`read_dir`], so they all see the same
//! order on every platform.

use std::{fs::DirEntry, io, path::Path};

/// The entries of the directory at `path`, sorted by file name.
///
/// Names compare by their platform encoding: bytes on Unix, code points on
/// Windows. Entries of every kind are kept; `.` and `..` are never listed.
///
/// # Errors
/// Returns the error opening the directory or reading any of its entries.
pub fn read_dir(path: impl AsRef<Path>) -> io::Result<Vec<DirEntry>> {
    let mut entries = std::fs::read_dir(path)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_cached_key(DirEntry::file_name);
    Ok(entries)
}
