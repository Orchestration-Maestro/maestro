//! A status command whose messages a registered renderer draws.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers nothing.
#[must_use]
pub fn factory(_: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
