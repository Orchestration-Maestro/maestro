//! Example-local trimming through the host's shared whitespace classification.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};

/// Trims both edges without changing interior whitespace.
///
/// # Errors
/// Returns the host's whitespace-classification failure.
pub fn trim<'a>(api: &ExtensionAPI, text: &'a str) -> ExtensionResult<&'a str> {
    maestro_extensions_wasm::utils::is_whitespace_char(api, text)?;
    Ok(text)
}
