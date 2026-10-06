use std::{ffi::OsString, future::Future, io, path::Path, pin::Pin, time::SystemTime};

/// Replaceable raw byte access at caller-supplied paths.
pub trait Storage {
    /// Observe target existence; failed observations return false.
    fn exists(&self, path: &Path) -> bool;
    /// Create the supplied directory and any missing parents.
    fn mkdir(&self, path: &Path) -> io::Result<()>;
    /// Read uninterpreted bytes without creating a file.
    fn read_file(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Open and read once at offset zero, returning at most the requested bytes.
    fn read_prefix(&self, path: &Path, length: usize) -> io::Result<Vec<u8>>;
    /// List names without per-member metadata observations.
    fn read_dir(&self, path: &Path) -> io::Result<Vec<OsString>>;
    /// Obtain the selected target modification time.
    fn modified(&self, path: &Path) -> io::Result<SystemTime>;
    /// Create or append the supplied bytes without framing.
    fn append_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Create or truncate in place, writing the supplied bytes.
    fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Admit an independent owned asynchronous byte read.
    fn read_file_async(
        &self,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'static>>;
    /// Admit an independent selected-target modification-time observation.
    fn modified_async(
        &self,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = io::Result<SystemTime>> + 'static>>;
    /// Admit an independent name-only directory listing.
    fn read_dir_async(
        &self,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<OsString>>> + 'static>>;
    /// Admit an independent listing with non-following entry kinds.
    fn read_dir_with_file_types_async(
        &self,
        path: &Path,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<Dirent>>> + 'static>>;
}

/// An owned directory entry with non-following classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirent {
    /// The directory member name, not its full path.
    pub name: OsString,
    /// The kind of the entry itself, not a symbolic-link target.
    pub kind: DirentKind,
}

/// Filesystem entry classifications without application filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirentKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// A symbolic link, including a dangling one.
    SymbolicLink,
    /// Any remaining filesystem kind.
    Other,
}
