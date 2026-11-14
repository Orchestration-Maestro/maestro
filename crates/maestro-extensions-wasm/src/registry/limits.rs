//! Supplied truncate tool data.
/// The limit reported as causing truncation.
pub use maestro_interchange::tool::tools::truncate::TruncationBy as TruncationLimit;
/// Supplied truncation output with floating-point accounting.
pub type TruncationResult = maestro_interchange::tool::tools::truncate::TruncationResult<f64>;

#[doc(inline)]
pub use maestro_interchange::tool::types::{TruncateLineResult, TruncationOptions};

/// Retains the head of tool output through the native owner, including all accounting.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn truncate_head(
    api: &crate::ExtensionAPI,
    content: &str,
    options: TruncationOptions,
) -> crate::ExtensionResult<TruncationResult> {
    api.helper(
        "tools.truncate_head",
        &(content, options.max_lines, options.max_bytes),
    )
}

/// Retains the tail of tool output through the native owner, including all accounting.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn truncate_tail(
    api: &crate::ExtensionAPI,
    content: &str,
    options: TruncationOptions,
) -> crate::ExtensionResult<TruncationResult> {
    api.helper(
        "tools.truncate_tail",
        &(content, options.max_lines, options.max_bytes),
    )
}

/// Clips a line through the native scalar-aware owner; `None` selects its default.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn truncate_line(
    api: &crate::ExtensionAPI,
    line: &str,
    max_chars: Option<usize>,
) -> crate::ExtensionResult<TruncateLineResult> {
    api.helper("tools.truncate_line", &(line, max_chars))
}

/// Formats the supplied byte count through the native size formatter.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn format_size(api: &crate::ExtensionAPI, bytes: f64) -> crate::ExtensionResult<String> {
    api.native_format_size(bytes)
}

/// Default native newline-entry limit.
pub const DEFAULT_MAX_LINES: usize = 2000;
/// Default native UTF-8 byte limit.
pub const DEFAULT_MAX_BYTES: usize = 51200;
/// Default native match-line scalar limit.
pub const GREP_MAX_LINE_LENGTH: usize = 500;
