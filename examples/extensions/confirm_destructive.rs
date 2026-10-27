//! Opt-in confirmation before replacing or forking a session.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Registers session confirmation handlers with the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.on(
            "session_before_switch",
            Rc::new(|_, _| Box::pin(async { Ok(None) })),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
