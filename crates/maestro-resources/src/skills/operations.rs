//! Replaceable supplied-path filesystem observations.

use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

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

/// Filesystem operations used by resource discovery.
pub trait ResourceOperations {
    /// Observe whether a path exists, following links.
    fn exists(&self, path: &Path) -> bool;
    /// Read directory entries in adapter-returned order, without following links.
    ///
    /// # Errors
    /// Returns the directory I/O cause.
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>>;
    /// Read text, replacing malformed UTF-8 with replacement characters.
    ///
    /// # Errors
    /// Returns the file I/O cause.
    fn read_file(&self, path: &Path) -> io::Result<String>;
    /// Observe a path's kind, following links.
    ///
    /// # Errors
    /// Returns the metadata I/O cause.
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType>;
    /// Resolve a real path.
    ///
    /// Native operations fold `.` and `..` lexically, then replace each link
    /// component by its target.
    ///
    /// # Errors
    /// Returns the I/O cause of the failed resolution.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
}

/// Standard native filesystem operations.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeResourceOperations;

#[cfg(not(target_arch = "wasm32"))]
impl ResourceOperations for NativeResourceOperations {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn read_dir(&self, path: &Path) -> io::Result<Vec<ResourceEntry>> {
        std::fs::read_dir(path)?
            .map(|entry| {
                let entry = entry?;
                Ok(ResourceEntry {
                    name: entry.file_name(),
                    file_type: file_type(entry.file_type()?),
                })
            })
            .collect()
    }
    fn read_file(&self, path: &Path) -> io::Result<String> {
        Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
    }
    fn metadata(&self, path: &Path) -> io::Result<ResourceFileType> {
        Ok(file_type(std::fs::metadata(path)?.file_type()))
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        super::real_path::real_path(path)
    }
}

/// Classify native directory-entry or metadata observations.
#[cfg(not(target_arch = "wasm32"))]
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
