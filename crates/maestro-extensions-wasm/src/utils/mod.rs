//! Host-backed stream and terminal text utilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub mod event_stream;

use crate::{ExtensionAPI, ExtensionResult};

/// Measures terminal cells, including the host's Unicode and escape handling.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn visible_width(api: &ExtensionAPI, text: &str) -> ExtensionResult<usize> {
    api.helper("visible_width", &(text,))
}

/// Normalizes terminal vowels through the host without changing ordinary newlines.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn normalize_terminal_output(api: &ExtensionAPI, text: &str) -> ExtensionResult<String> {
    api.helper("normalize_terminal_output", &(text,))
}

/// A supported escape at a UTF-8 byte boundary.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnsiCode {
    /// Escape text exactly as supplied.
    pub code: String,
    /// Length in UTF-8 bytes.
    pub length: usize,
}

/// Reads an escape beginning at the supplied UTF-8 byte offset.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn extract_ansi_code(
    api: &ExtensionAPI,
    text: &str,
    pos: usize,
) -> ExtensionResult<Option<AnsiCode>> {
    api.helper("extract_ansi_code", &(text, pos))
}

/// Checks the host whitespace set against every supplied scalar.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_whitespace_char(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    api.helper("is_whitespace_char", &(text,))
}

/// Checks the host punctuation set against every supplied scalar.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_punctuation_char(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    api.helper("is_punctuation_char", &(text,))
}
