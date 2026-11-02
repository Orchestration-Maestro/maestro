//! Owned native filesystem notifications.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{CallbackEmitter, ExtensionResult};
use std::rc::Rc;

/// A retained file notification callback with its invocation-local emitter.
pub type WatchListener = Rc<dyn for<'a> Fn(CallbackEmitter<'a>) -> ExtensionResult<()>>;

/// An independently owned native watch registration.
pub trait FsWatcher {
    /// Stops delivery and releases the registration; repeated close is harmless.
    ///
    /// # Errors
    /// Returns the native close failure.
    fn close(&self) -> ExtensionResult<()>;
}
