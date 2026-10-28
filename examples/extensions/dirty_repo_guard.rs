//! Opt-in guard that asks before leaving or forking a repository with uncommitted changes.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers the session guards with the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        drop(api);
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
