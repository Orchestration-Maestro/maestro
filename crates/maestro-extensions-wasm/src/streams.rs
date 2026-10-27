//! Independently retained raw byte streams.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult};

/// A consuming raw-byte reader; a pending read is distinct from EOF.
pub trait Readable {
    /// Reads the next available bytes, or `None` at EOF.
    fn read(&self) -> ExtensionFuture<'_, Option<Vec<u8>>>;
    /// Releases this consumer without cancelling its caller.
    ///
    /// # Errors
    /// Returns the native close failure.
    fn close(&self) -> ExtensionResult<()>;
}

/// An ordered raw-byte writer.
pub trait Writable {
    /// Writes every supplied byte, including across partial native writes.
    fn write<'a>(&'a self, bytes: &'a [u8]) -> ExtensionFuture<'a, ()>;
    /// Closes input after already admitted writes.
    fn end(&self) -> ExtensionFuture<'_, ()>;
}
