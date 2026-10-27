//! Opt-in registration of a custom message-stream provider.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Loads the opt-in provider through the component's public extension interface.
pub fn custom_message_provider_extension(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

maestro_extensions_wasm::export_extension!(custom_message_provider_extension);
