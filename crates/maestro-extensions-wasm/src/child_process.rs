//! Native child controls and descriptor selections.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    ExtensionFuture, ExtensionResult,
    streams::{Readable, Writable},
};
use std::rc::Rc;

#[doc(inline)]
pub use maestro_interchange::native_io::{ExitStatus, SpawnOptions, Stdio};

/// Child controls and byte pipes that can outlive their creating callback.
pub trait ChildProcess {
    /// The PID of the admitted child.
    fn pid(&self) -> Option<u32>;
    /// The input pipe, absent unless selected.
    fn stdin(&self) -> Option<Rc<dyn Writable>>;
    /// The output pipe, absent unless selected.
    fn stdout(&self) -> Option<Rc<dyn Readable>>;
    /// The error pipe, absent unless selected.
    fn stderr(&self) -> Option<Rc<dyn Readable>>;
    /// Waits for numeric exit or signal termination and both output-close phases.
    fn wait(&self) -> ExtensionFuture<'_, ExitStatus>;
    /// Signals this child, returning false when it already exited.
    ///
    /// # Errors
    /// Returns the native signal failure.
    fn kill(&self, signal: Option<String>) -> ExtensionResult<bool>;
}
