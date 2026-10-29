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
pub fn trim<'a>(api: &ExtensionAPI, mut text: &'a str) -> ExtensionResult<&'a str> {
    while let Some(ch) = text.chars().next() {
        if !maestro_extensions_wasm::utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[ch.len_utf8()..];
    }
    while let Some(ch) = text.chars().next_back() {
        if !maestro_extensions_wasm::utils::is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[..text.len() - ch.len_utf8()];
    }
    Ok(text)
}
