//! Typed native byte and child imports used by the component adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult};

/// Native resource operations implemented by generated and controlled imports.
pub trait NativeImports {
    /// Retained native child identity.
    type Child: 'static;
    /// Retained raw reader identity.
    type Readable: 'static;
    /// Retained raw writer identity.
    type Writable: 'static;
    /// Launches an owned native child.
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, Self::Child>;
    /// Signals an arbitrary native PID or group.
    fn kill(&self, pid: i32, signal: Option<String>) -> ExtensionResult<()>;
    /// Reads the admitted child's PID.
    fn child_pid(&self, child: &Self::Child) -> Option<u32>;
    /// Acquires the admitted input pipe without replacing it.
    fn child_stdin(&self, child: &Self::Child) -> Option<Self::Writable>;
    /// Acquires the admitted output pipe without replacing it.
    fn child_stdout(&self, child: &Self::Child) -> Option<Self::Readable>;
    /// Acquires the admitted error pipe without replacing it.
    fn child_stderr(&self, child: &Self::Child) -> Option<Self::Readable>;
    /// Waits for this child's output close and status.
    fn child_wait<'a>(
        &'a self,
        child: &'a Self::Child,
    ) -> ExtensionFuture<'a, crate::child_process::ExitStatus>;
    /// Signals this admitted child only while it is live.
    fn child_kill(&self, child: &Self::Child, signal: Option<String>) -> ExtensionResult<bool>;
    /// Reads one native byte chunk.
    fn readable_read<'a>(
        &'a self,
        reader: &'a Self::Readable,
    ) -> ExtensionFuture<'a, Option<Vec<u8>>>;
    /// Releases a raw consumer.
    fn readable_close(&self, reader: &Self::Readable) -> ExtensionResult<()>;
    /// Writes all the admitted bytes.
    fn writable_write<'a>(
        &'a self,
        writer: &'a Self::Writable,
        bytes: &'a [u8],
    ) -> ExtensionFuture<'a, ()>;
    /// Closes input after admitted writes.
    fn writable_end<'a>(&'a self, writer: &'a Self::Writable) -> ExtensionFuture<'a, ()>;
}
