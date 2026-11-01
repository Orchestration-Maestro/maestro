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

/// Selection of one child descriptor.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Stdio {
    /// An independently retained byte pipe.
    #[default]
    Pipe,
    /// The native null device.
    Ignore,
    /// The host's corresponding descriptor.
    Inherit,
}

/// Literal selections for one child launch.
#[derive(Default)]
pub struct SpawnOptions {
    /// Omitted inherits the host directory.
    pub cwd: Option<String>,
    /// The entire environment; present-empty replaces it with an empty environment.
    pub env: Option<Vec<(String, String)>>,
    /// Creates a new native session where the host supports it.
    pub detached: bool,
    /// Independent stdin, stdout and stderr selections.
    pub stdio: [Stdio; 3],
}

/// Native numeric exit or terminating signal, kept distinct.
#[derive(Debug, Eq, PartialEq)]
pub struct ExitStatus {
    /// Absent for signal termination.
    pub code: Option<i32>,
    /// Absent for numeric exit.
    pub signal: Option<String>,
}

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
