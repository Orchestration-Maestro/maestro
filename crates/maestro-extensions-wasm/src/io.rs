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
    /// Waits for the selected duration, failing with `Aborted` if the signal aborts.
    fn sleep(&self, duration: Duration, signal: Option<AbortSignal>) -> ExtensionFuture<'_, ()>;
}
