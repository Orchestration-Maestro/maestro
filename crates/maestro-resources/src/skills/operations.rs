//! Replaceable supplied-path filesystem observations.

use maestro_path::{Cwd, resolve, try_resolve};
use std::{
    cell::OnceCell,
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
    /// Resolution continues from it when no operand anchors a path. Each
    /// lexical-resolution context asks at most once, at its first resolution
    /// whose operands leave a part open, together with
    /// [`drive_directories`](Self::drive_directories), and keeps the outcome, a
    /// failure included, for the rest of that context. A loader call has one
    /// context; native [`canonicalize`](Self::canonicalize) starts its own for
    /// each path it resolves, so one loader call can ask more than once. A
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

/// The adapter's process directories, which a path resolution continues from when its operands leave a part open.
///
/// The adapter is asked for them once, at the first resolution whose operands
/// leave a part open, and every later resolution of the same load reuses what
/// that observation returned, a failure to read the working directory included.
/// A resolution whose operands leave open a part that only that directory
/// supplies fails with the failure's kind; one the operands anchor, or that a
/// drive directory completes, does not depend on it. A caller's working
/// directory is an operand of each resolution.
pub(crate) struct ProcessContext<'a> {
    /// The adapter observed on first need.
    operations: &'a dyn ResourceOperations,
    /// The working directory, or the kind of the error that reading it returned.
    current: OnceCell<Result<String, io::ErrorKind>>,
    /// The current directory of each drive.
    drives: OnceCell<Vec<(char, String)>>,
}

impl<'a> ProcessContext<'a> {
    /// A context that has not yet asked `operations` for anything.
    pub(crate) fn new(operations: &'a dyn ResourceOperations) -> Self {
        Self {
            operations,
            current: OnceCell::new(),
            drives: OnceCell::new(),
        }
    }

    /// Hand `use_directories` the working directory and drive directories, asking the adapter the first time.
    pub(super) fn observe<R>(
        &self,
        use_directories: impl FnOnce(Result<&str, io::ErrorKind>, &[(char, &str)]) -> R,
    ) -> R {
        let current = self.current.get_or_init(|| {
            self.operations
                .current_directory()
                .map_err(|error| error.kind())
        });
        let drives = self
            .drives
            .get_or_init(|| self.operations.drive_directories());
        let drives: Vec<(char, &str)> = drives
            .iter()
            .map(|(letter, directory)| (*letter, directory.as_str()))
            .collect();
        use_directories(current.as_deref().map_err(|kind| *kind), &drives)
    }

    /// Resolve `paths` from right to left, asking the adapter for its directories only if they leave a part open.
    ///
    /// # Errors
    /// Returns the kind of the working directory's error when it was needed and unreadable.
    pub(crate) fn resolve(&self, paths: &[&str]) -> io::Result<String> {
        try_resolve(paths, &[]).or_else(|_| {
            self.observe(|current, drives| match current {
                Ok(current) => Ok(resolve(
                    paths,
                    &Cwd {
                        current,
                        drive_directories: drives,
                    },
                )),
                Err(kind) => try_resolve(paths, drives).map_err(|_| kind.into()),
            })
        })
    }
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
