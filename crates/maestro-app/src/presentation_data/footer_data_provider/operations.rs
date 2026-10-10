//! The file-system and process effects footer metadata needs.

use std::{io, rc::Rc, time::Duration};

use maestro_watch::fs_watch::{FsWatcher, WatchOperations};

/// What a path names after following symbolic links.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterFileKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// Anything else, such as a socket or device.
    Other,
}

/// The three metadata values selected by footer polling; a failed stat uses zero.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FooterFileStats {
    /// Modification time in milliseconds.
    pub mtime_ms: f64,
    /// Metadata change time in milliseconds.
    pub ctime_ms: f64,
    /// File size in bytes.
    pub size: f64,
}
/// Replaceable footer effects; branch selection and trimming stay with the provider.
pub trait FooterOperations: WatchOperations {
    /// Complete once with untrimmed stdout, absence on unsuccessful exit, or an I/O error.
    fn symbolic_ref_async(
        &self,
        repo_dir: &str,
        callback: Box<dyn FnOnce(io::Result<Option<String>>)>,
    );
    /// Poll current and previous samples through an independently closable registration.
    fn watch_file(
        &self,
        path: &str,
        interval: Duration,
        listener: Rc<dyn Fn(FooterFileStats, FooterFileStats)>,
    ) -> Box<dyn FsWatcher>;
    /// Whether `path` exists; an error while checking counts as absent.
    fn exists(&self, path: &str) -> bool;
    /// The kind of `path`, following symbolic links.
    ///
    /// # Errors
    /// Returns the failure to read the path's metadata.
    fn stat_kind(&self, path: &str) -> io::Result<FooterFileKind>;
    /// The text of the file at `path`, with invalid UTF-8 replaced by U+FFFD.
    ///
    /// # Errors
    /// Returns the failure to read the file.
    fn read_text(&self, path: &str) -> io::Result<String>;
    /// The process working directory, read only when resolving a path needs it.
    ///
    /// # Errors
    /// Returns the failure to read the working directory.
    fn current_dir(&self) -> io::Result<String>;
    /// The current directory of a Windows drive, when one is recorded.
    fn drive_directory(&self, drive: char) -> Option<String>;
    /// Run `git --no-optional-locks symbolic-ref --quiet --short HEAD` in
    /// `repo_dir` and return its untrimmed standard output.
    ///
    /// A process that does not exit successfully gives `Ok(None)`.
    ///
    /// # Errors
    /// Returns the failure to start the process.
    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>>;
}
