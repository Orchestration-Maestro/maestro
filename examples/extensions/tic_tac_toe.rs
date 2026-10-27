//! Play tic-tac-toe through a command and sequential atomic tools.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Registers the opt-in game.
///
/// # Errors
/// Returns registration failures from the host.
#[must_use]
pub fn factory(
    _: maestro_extensions_wasm::ExtensionAPI,
) -> maestro_extensions_wasm::ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
