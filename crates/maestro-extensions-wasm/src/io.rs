//! Native operations supplied by the extension's host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::child_process::{ChildProcess, SpawnOptions};
use crate::{AbortSignal, ExtensionFuture};
use std::{rc::Rc, time::Duration};

/// Owned access to native operations, independent of a callback's lifetime.
pub trait NativeIo {
    /// Atomically publishes bytes, preserving existing permissions.
    ///
    /// New files use `mode`; success carries an optional durability warning.
    /// Final symbolic links are followed, including dangling targets.
    ///
    /// # Errors
    /// Returns the native failure before publication.
    fn save_file(&self, path: &str, bytes: &[u8], mode: u32) -> Result<Option<String>, String>;
    /// Reads native Unix permission bits, including file type bits.
    ///
    /// # Errors
    /// Returns native metadata errors or unsupported on non-Unix hosts.
    fn file_mode(&self, path: &str) -> Result<u32, String>;
    /// Changes native Unix permission bits, following symbolic links.
    ///
    /// # Errors
    /// Returns native permission errors or unsupported on non-Unix hosts.
    fn set_file_mode(&self, path: &str, mode: u32) -> Result<(), String>;
    /// Creates directories recursively with the requested Unix creation mode.
    ///
    /// Existing directories are unchanged. Windows does not configure ACLs.
    ///
    /// # Errors
    /// Returns native directory creation errors.
    fn create_dir(&self, path: &str, mode: u32) -> Result<(), String>;

    /// Watches a supplied file until explicitly closed or its last handle is dropped.
    ///
    /// # Errors
    /// Returns native watch creation failures without retaining the callback.
    fn watch(
        &self,
        path: String,
        listener: crate::fs::WatchListener,
    ) -> crate::ExtensionResult<Rc<dyn crate::fs::FsWatcher>>;
    /// Launches a literal program and argument vector without selecting a shell.
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: SpawnOptions,
    ) -> ExtensionFuture<'_, Rc<dyn ChildProcess>>;
    /// Signals a native PID, or a process group on hosts that support it.
    ///
    /// # Errors
    /// Returns the native signal failure.
    fn kill(&self, pid: i32, signal: Option<String>) -> crate::ExtensionResult<()>;
    /// Sends one HTTP attempt without retry or an implicit timeout.
    fn fetch(
        &self,
        request: crate::http::Request,
    ) -> ExtensionFuture<'_, Rc<dyn crate::http::Response>>;
    /// Waits for the selected duration, failing with `Aborted` if the signal aborts.
    fn sleep(&self, duration: Duration, signal: Option<AbortSignal>) -> ExtensionFuture<'_, ()>;
}
