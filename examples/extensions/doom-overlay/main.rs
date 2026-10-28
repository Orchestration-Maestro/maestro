//! Play DOOM in a retained terminal overlay.
#![forbid(clippy::pedantic, clippy::too_many_arguments, clippy::excessive_nesting)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the overlay command.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
