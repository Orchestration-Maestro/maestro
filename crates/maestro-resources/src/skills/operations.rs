//! Replaceable supplied-path filesystem observations.

use maestro_path::{Cwd, relative, resolve, try_resolve};
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
    /// Native operations fold `.` and `..` lexically and replace each link
    /// component by its target until none remains. They fail when a component
    /// cannot be inspected, a link's target is missing or loops, a link is
    /// expanded again before the walk has read any component of what followed
    /// it at its previous expansion, or the path needs a working directory
    /// that cannot be read.
    ///
    /// # Errors
    /// Returns the I/O cause of the failed resolution.
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    /// Observe the process working directory.
    ///
    /// Resolution continues from it when no operand anchors a path. A
    /// resolution that needs it while this fails fails with the same error kind.
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

/// The adapter's process directories, which every path resolution continues from.
///
/// The working directory is kept as the adapter reported it, including a failure
/// to read it. A resolution whose operands leave a part open that only that
/// directory supplies fails with the failure's kind; one the operands anchor, or
/// that a drive directory completes, does not read it. A caller's working
/// directory is an operand of each resolution.
#[derive(Clone, Copy)]
pub(super) struct ProcessContext<'a> {
    /// The working directory, or the kind of the error that reading it returned.
    pub(super) current: Result<&'a str, io::ErrorKind>,
    /// The current directory of each drive.
    pub(super) drives: &'a [(char, &'a str)],
}

impl<'a> ProcessContext<'a> {
    /// The snapshot that continues from `current`.
    fn cwd(&self, current: &'a str) -> Cwd<'a> {
        Cwd {
            current,
            drive_directories: self.drives,
        }
    }

    /// Resolve `paths` from right to left, reading the working directory only if they leave a part open.
    ///
    /// # Errors
    /// Returns the kind of the working directory's error when it was needed and unreadable.
    pub(super) fn resolve(&self, paths: &[&str]) -> io::Result<String> {
        match self.current {
            Ok(current) => Ok(resolve(paths, &self.cwd(current))),
            Err(kind) => try_resolve(paths, self.drives).map_err(|_| kind.into()),
        }
    }

    /// The path from `from` to `to`, which are already resolved.
    ///
    /// Neither end is relative, so the working directory is not read; `from`
    /// fills the snapshot that `maestro_path::relative` takes.
    pub(super) fn relative(&self, from: &str, to: &str) -> String {
        relative(from, to, &self.cwd(from))
    }
}

/// Run `operation` with the adapter's process directories as the resolution context.
pub(super) fn with_process_context<R>(
    operations: &dyn ResourceOperations,
    operation: impl FnOnce(ProcessContext<'_>) -> R,
) -> R {
    let current = operations.current_directory();
    let entries = operations.drive_directories();
    let drives: Vec<(char, &str)> = entries
        .iter()
        .map(|(letter, directory)| (*letter, directory.as_str()))
        .collect();
    operation(ProcessContext {
        current: current.as_deref().map_err(io::Error::kind),
        drives: &drives,
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
