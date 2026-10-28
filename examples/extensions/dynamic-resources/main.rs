//! Contributes a skill, prompt template and theme installed beside the component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the sibling resources during discovery.
#[must_use]
pub fn dynamic_resources_extension(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(dynamic_resources_extension);
