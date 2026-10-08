//! Replaceable supplied-path filesystem observations.

use maestro_path::Cwd;
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
    /// Native operations fold `.` and `..` lexically, replace each link
    /// component by its target and fail once a walk needs more link expansions
    /// than the platform allows.
    ///
    /// # Errors
    /// Returns the I/O cause of the failed resolution.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    /// Observe the process working directory.
    ///
    /// Resolution continues from it when no operand anchors a path.
    ///
    /// # Errors
    /// Returns the I/O cause when the directory cannot be read.
    fn current_directory(&self) -> io::Result<String>;
    /// Observe the current directory of each Windows drive, keyed by uppercase letter.
    ///
    /// Path resolution hands these entries to `maestro_path::Cwd`. The Windows
    /// flavor continues a drive-relative path such as `D:item.md` from its
    /// drive's entry unless an earlier operand anchors that drive; the POSIX
    /// flavor ignores them. Adapters without per-drive directories return no
    /// entries.
    fn drive_directories(&self) -> Vec<(char, String)>;
}

/// Run `operation` with the adapter's process directories as the resolution context.
///
/// This is the only place a `Cwd` is built. The context holds process state
/// only: a caller's working directory is an operand of each resolution, and the
/// context supplies what its operands leave open. A working directory that cannot
/// be read is empty: a path then resolves to an absolute path only when its
/// operands or a drive directory anchor it.
pub(super) fn with_process_context<R>(
    operations: &dyn ResourceOperations,
    operation: impl FnOnce(&Cwd<'_>) -> R,
) -> R {
    let current = operations.current_directory().unwrap_or_default();
    let entries = operations.drive_directories();
    let drives: Vec<(char, &str)> = entries
        .iter()
        .map(|(letter, directory)| (*letter, directory.as_str()))
        .collect();
    operation(&Cwd {
        current: &current,
        drive_directories: &drives,
    })
}

/// Standard native filesystem operations.
///
/// The process working directory is the operating system's. On Windows the
/// drive directories are the `=X:` environment variables of the drives that have
/// one; elsewhere there are none.
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
        super::real_path::real_path(path, self)
    }
    fn current_directory(&self) -> io::Result<String> {
        Ok(std::env::current_dir()?.to_string_lossy().into_owned())
    }
    fn drive_directories(&self) -> Vec<(char, String)> {
        if !cfg!(windows) {
            return Vec::new();
        }
        env_drive_directories(|name| std::env::var_os(name))
    }
}

/// Read each drive's `=X:` variable through `var`, in letter order.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn env_drive_directories(var: impl Fn(&str) -> Option<OsString>) -> Vec<(char, String)> {
    ('A'..='Z')
        .filter_map(|letter| {
            var(&format!("={letter}:")).map(|dir| (letter, dir.to_string_lossy().into_owned()))
        })
        .collect()
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
