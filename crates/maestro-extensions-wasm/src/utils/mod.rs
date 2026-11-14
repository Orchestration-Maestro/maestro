//! Host-backed clock, stream and terminal text utilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub mod events;

use crate::{ExtensionAPI, ExtensionResult};

/// Reads the host's current local date and time using its selected locale.
///
/// The host supplies the local time zone.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn local_date_time_string(api: &ExtensionAPI) -> ExtensionResult<String> {
    api.helper("time.local_date_time_string", &())
}

/// Formats an instant, in milliseconds since the Unix epoch, as the host's local time in the
/// en-US twelve-hour format.
///
/// The host supplies its local time zone; an instant outside its calendar reads as
/// `Invalid Date`.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn local_time_string(api: &ExtensionAPI, timestamp: i64) -> ExtensionResult<String> {
    api.helper("time.local_time_string", &(timestamp,))
}

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

/// Reports whether any supplied scalar is in the host whitespace set.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_whitespace_char(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    api.helper("is_whitespace_char", &(text,))
}

/// Trims both edges while the host classifies each edge scalar as whitespace.
///
/// Interior whitespace, newlines included, stays as it is, and the returned slice borrows
/// `text`.
///
/// # Errors
/// Returns the host's whitespace-classification failure.
pub fn trim<'a>(api: &ExtensionAPI, mut text: &'a str) -> ExtensionResult<&'a str> {
    while let Some(ch) = text.chars().next() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[ch.len_utf8()..];
    }
    while let Some(ch) = text.chars().next_back() {
        if !is_whitespace_char(api, &ch.to_string())? {
            break;
        }
        text = &text[..text.len() - ch.len_utf8()];
    }
    Ok(text)
}

/// Reports whether any supplied scalar is in the host punctuation set.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn is_punctuation_char(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    api.helper("is_punctuation_char", &(text,))
}

/// Wraps text at terminal cells while preserving visible ANSI styling.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn wrap_text_with_ansi(
    api: &ExtensionAPI,
    text: &str,
    width: usize,
) -> ExtensionResult<Vec<String>> {
    api.helper("wrap_text_with_ansi", &(text, width))
}

/// Passes the host-padded line to `bg_fn` exactly once before returning.
/// The borrowed callback is never stored or called after an error return.
///
/// # Errors
/// Returns the host's failure before invoking the callback.
pub fn apply_background_to_line(
    api: &ExtensionAPI,
    line: &str,
    width: usize,
    bg_fn: impl FnOnce(&str) -> String,
) -> ExtensionResult<String> {
    let padded: String = api.helper("apply_background_to_line", &(line, width))?;
    Ok(bg_fn(&padded))
}

pub use maestro_interchange::tui::{ColumnSlice, ExtractedSegments, TruncateOptions};

/// Truncates text through the native terminal owner, preserving whole clusters and styling.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn truncate_to_width(
    api: &ExtensionAPI,
    text: &str,
    max_width: usize,
    options: TruncateOptions<'_>,
) -> ExtensionResult<String> {
    api.helper(
        "truncate_to_width",
        &(text, max_width, options.ellipsis, options.pad),
    )
}

/// Selects whole graphemes by terminal columns, optionally excluding right-edge crossings.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn slice_with_width(
    api: &ExtensionAPI,
    line: &str,
    start_col: usize,
    length: usize,
    strict: bool,
) -> ExtensionResult<ColumnSlice> {
    api.helper("slice_with_width", &(line, start_col, length, strict))
}

/// Returns the text of a terminal-column selection.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn slice_by_column(
    api: &ExtensionAPI,
    line: &str,
    start_col: usize,
    length: usize,
    strict: bool,
) -> ExtensionResult<String> {
    Ok(slice_with_width(api, line, start_col, length, strict)?.text)
}

/// Terminal-column boundaries; strictness applies only to the after side.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct SegmentSelection {
    /// First column excluded from the before side.
    pub before_end: usize,
    /// First column selected for the after side.
    pub after_start: usize,
    /// Maximum columns selected for the after side.
    pub after_len: usize,
    /// Whether a grapheme crossing the after boundary is excluded.
    pub strict_after: bool,
}

/// Extracts both sides through the host while retaining their independent widths.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn extract_segments(
    api: &ExtensionAPI,
    line: &str,
    selection: SegmentSelection,
) -> ExtensionResult<ExtractedSegments> {
    api.helper(
        "extract_segments",
        &(
            line,
            selection.before_end,
            selection.after_start,
            selection.after_len,
            selection.strict_after,
        ),
    )
}
