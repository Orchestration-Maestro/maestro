//! Native operations supplied by the extension's host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AbortSignal, ExtensionFuture};
use std::time::Duration;

/// Owned access to native operations, independent of a callback's lifetime.
pub trait NativeIo {
    /// Waits for the selected duration, failing with `Aborted` if the signal aborts.
    fn sleep(&self, duration: Duration, signal: Option<AbortSignal>) -> ExtensionFuture<'_, ()>;
}
