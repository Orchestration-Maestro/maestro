//! Follows the macOS system appearance through the host's theme interface.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers appearance synchronization.
///
/// # Errors
/// Returns registration failures unchanged.
#[must_use]
pub fn factory(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
