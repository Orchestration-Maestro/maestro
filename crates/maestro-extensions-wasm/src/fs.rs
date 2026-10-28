//! Native filesystem access through the retained host capability.
//!
//! Obtain [`crate::io::NativeIo`] from [`crate::ExtensionAPI::native_io`]. Its
//! `save_file`, `file_mode`, `set_file_mode` and `create_dir` methods use authored
//! native paths directly, without converting them for the guest standard library.
//! Saves return an optional postcommit durability warning, not a second failure.
//! Unix hosts expose native permission bits; other hosts reject mode reads and
//! changes rather than inventing POSIX bits. Watches retain their own lifetime.
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
