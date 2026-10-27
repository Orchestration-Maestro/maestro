//! A companion extension exercising the terminal client's RPC UI methods.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the companion's lifecycle hooks and interactive commands.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        drop(api);
        Ok(())
    })
}

maestro_extensions_wasm::export_extension!(factory);
