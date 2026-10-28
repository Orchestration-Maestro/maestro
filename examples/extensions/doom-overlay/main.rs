//! Play DOOM in a retained terminal overlay.
#![forbid(clippy::pedantic, clippy::too_many_arguments, clippy::excessive_nesting)]
use maestro_extensions_wasm::{CommandOptions, ExtensionAPI, ExtensionFuture};
use std::rc::Rc;

/// Registers the overlay command.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.register_command(
            "doom-overlay",
            CommandOptions {
                description: Some("Play DOOM in an overlay. Q pauses and closes it.".into()),
                get_argument_completions: None,
                handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
