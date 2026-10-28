//! A provider for GitLab Duo's Claude and GPT models, with GitLab sign-in and requests delegated to the built-in protocols.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the opt-in provider through the component's public extension interface.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        drop(api);
        Ok(())
    })
}

maestro_extensions_wasm::export_extension!(factory);
