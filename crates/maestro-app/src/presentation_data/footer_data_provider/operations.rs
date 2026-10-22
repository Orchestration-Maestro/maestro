//! The watch registration, polling and Git process effects footer metadata needs;
//! repository discovery, the `HEAD` read and the presence checks read the file system
//! directly.

use std::{io, rc::Rc, time::Duration};

use maestro_watch::fs_watch::{FsWatcher, WatchOperations};

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
/// Replaceable watch registration, polling and Git process effects; repository
/// discovery, the `HEAD` read and the presence checks read the file system directly, and
/// branch selection and trimming stay with the provider.
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
    /// Run `git --no-optional-locks symbolic-ref --quiet --short HEAD` in
    /// `repo_dir` and return its untrimmed standard output.
    ///
    /// A process that does not exit successfully gives `Ok(None)`.
    ///
    /// # Errors
    /// Returns the failure to start the process.
    fn symbolic_ref_sync(&self, repo_dir: &str) -> io::Result<Option<String>>;
}
