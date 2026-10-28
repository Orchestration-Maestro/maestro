//! Shows the model, thinking level, context use, working directory and Git branch on the
//! editor's borders, in place of the footer and the working row.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers nothing yet.
///
/// # Errors
/// Never fails.
#[must_use]
pub fn factory(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
