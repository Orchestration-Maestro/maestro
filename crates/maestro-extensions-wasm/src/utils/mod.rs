//! Host-backed clock, stream and terminal text utilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub mod event_stream;

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

/// The ellipsis and padding used by terminal-width truncation.
#[derive(Clone, Copy, Debug)]
pub struct TruncateOptions<'a> {
    /// Appended marker, measured in terminal cells.
    pub ellipsis: &'a str,
    /// Whether the completed result fills the requested cell width.
    pub pad: bool,
}
impl Default for TruncateOptions<'_> {
    fn default() -> Self {
        Self {
            ellipsis: "...",
            pad: false,
        }
    }
}

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

/// Text selected by terminal columns and its actual cell width.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ColumnSlice {
    /// Selected text with its styling.
    pub text: String,
    /// Cells occupied by that text.
    pub width: usize,
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

/// Both sides of an excluded terminal-column range.
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSegments {
    /// Text before the excluded range.
    pub before: String,
    /// Cells occupied by the before text.
    pub before_width: usize,
    /// Text after the excluded range.
    pub after: String,
    /// Cells occupied by the after text.
    pub after_width: usize,
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
