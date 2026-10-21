//! Filesystem observations of resource discovery.

use maestro_path::resolve_with;
use std::{ffi::OsString, io, path::Path};

/// The observed kind of a resource entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceFileType {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A link whose target has not been inspected.
    Symlink,
    /// Another filesystem object.
    Other,
}

/// One directory entry without following its links.
#[derive(Debug)]
pub struct ResourceEntry {
    /// Authored entry name.
    pub name: OsString,
    /// Entry kind before following symlinks.
    pub file_type: ResourceFileType,
}

/// The entries of a directory in name order, without following links.
///
/// # Errors
/// Returns the directory I/O cause.
#[cfg(not(target_arch = "wasm32"))]
pub fn read_entries(path: &Path) -> io::Result<Vec<ResourceEntry>> {
    maestro_listing::read_dir(path)?
        .into_iter()
        .map(|entry| {
            Ok(ResourceEntry {
                name: entry.file_name(),
                file_type: file_type(entry.file_type()?),
            })
        })
        .collect()
}

/// Browser builds have no file system, so no directory can be listed.
///
/// # Errors
/// Always returns `Unsupported`.
#[cfg(target_arch = "wasm32")]
pub fn read_entries(_path: &Path) -> io::Result<Vec<ResourceEntry>> {
    Err(io::ErrorKind::Unsupported.into())
}

/// Read text, replacing malformed UTF-8 with replacement characters.
///
/// # Errors
/// Returns the file I/O cause.
pub fn read_text(path: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// Observe a path's kind, following links.
///
/// # Errors
/// Returns the metadata I/O cause.
pub fn metadata(path: &Path) -> io::Result<ResourceFileType> {
    Ok(file_type(std::fs::metadata(path)?.file_type()))
}

/// Observe a path's kind without following its final link.
///
/// # Errors
/// Returns the metadata I/O cause.
pub fn symlink_metadata(path: &Path) -> io::Result<ResourceFileType> {
    Ok(file_type(std::fs::symlink_metadata(path)?.file_type()))
}

/// The current directory of each Windows drive, keyed by uppercase letter: the `=X:`
/// environment variables of the drives that have one. Other platforms have none.
#[must_use]
pub fn drive_directories() -> Vec<(char, String)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    maestro_path::env_drive_directories(|name| std::env::var_os(name))
}

/// The process working directory.
///
/// # Errors
/// Returns the I/O cause when the directory cannot be read.
pub fn current_directory() -> io::Result<String> {
    Ok(std::env::current_dir()?.to_string_lossy().into_owned())
}

/// Resolve `paths` from right to left, reading the process working directory and the
/// drive directories only when the operands leave a part open.
///
/// Each resolution reads them anew.
///
/// # Errors
/// Returns the kind of the working directory's error when it was needed and unreadable.
pub(crate) fn resolve(paths: &[&str]) -> io::Result<String> {
    resolve_with(&[paths], drive_directories, || {
        current_directory().map_err(|error| io::Error::from(error.kind()))
    })
    .map(|resolved| resolved.into_iter().collect::<String>())
}

/// Classify directory-entry or metadata observations.
fn file_type(value: std::fs::FileType) -> ResourceFileType {
    if value.is_file() {
        ResourceFileType::File
    } else if value.is_dir() {
        ResourceFileType::Directory
    } else if value.is_symlink() {
        ResourceFileType::Symlink
    } else {
        ResourceFileType::Other
    }
}
