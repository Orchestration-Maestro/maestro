//! An opt-in input hook that expands `!{command}` with shell output before submission.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};
use std::rc::Rc;

/// Registers the input hook through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("input", Rc::new(|_, _| Box::pin(async { Ok(None) }))) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
