//! Byte and line truncation of tool output.

use serde::Serialize;

/// Default line budget.
pub const DEFAULT_MAX_LINES: usize = 2000;
/// Default UTF-8 byte budget.
pub const DEFAULT_MAX_BYTES: usize = 51200;
/// Default scalar budget for match-line clipping.
pub const GREP_MAX_LINE_LENGTH: usize = 500;

/// Independent output limits; absent fields select their defaults.
#[derive(Clone, Copy, Default)]
pub struct TruncationOptions {
    /// Maximum retained entries; `usize::MAX` leaves this budget unbounded.
    pub max_lines: Option<usize>,
    /// Maximum retained UTF-8 bytes; `usize::MAX` leaves this budget unbounded.
    pub max_bytes: Option<usize>,
}

/// The limit that stopped selection.
pub use maestro_tool::tools::truncate::TruncationBy;
/// Selected output with native counts.
pub type TruncationResult = maestro_tool::tools::truncate::TruncationResult<usize>;

/// Account for input lines, excluding the empty entry after a final newline.
fn initial_result(content: &str, options: TruncationOptions) -> TruncationResult {
    let total_lines = content.split_terminator('\n').count();
    let total_bytes = content.len();
    let max_lines = options.max_lines.unwrap_or(DEFAULT_MAX_LINES);
    let max_bytes = options.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let fits = total_lines <= max_lines && total_bytes <= max_bytes;
    TruncationResult {
        content: if fits { content.into() } else { String::new() },
        truncated: false,
        truncated_by: None,
        total_lines,
        total_bytes,
        output_lines: total_lines,
        output_bytes: total_bytes,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines,
        max_bytes,
    }
}
/// Retain complete leading newline-separated entries.
#[must_use]
pub fn truncate_head(content: &str, options: TruncationOptions) -> TruncationResult {
    let mut result = initial_result(content, options);
    if result.total_lines <= result.max_lines && result.total_bytes <= result.max_bytes {
        return result;
    }
    if content
        .split_terminator('\n')
        .next()
        .is_some_and(|entry| entry.len() > result.max_bytes)
    {
        result.output_lines = 0;
        result.output_bytes = 0;
        result.truncated = true;
        result.truncated_by = Some(TruncationBy::Bytes);
        result.first_line_exceeds_limit = true;
        return result;
    }
    let mut selected = Vec::new();
    let mut bytes = 0;
    let mut reason = TruncationBy::Lines;
    for entry in content.split_terminator('\n').take(result.max_lines) {
        let cost = entry.len() + usize::from(!selected.is_empty());
        if cost > result.max_bytes - bytes {
            reason = TruncationBy::Bytes;
            break;
        }
        selected.push(entry);
        bytes += cost;
    }
    if matches!(reason, TruncationBy::Lines) && selected.len() == result.total_lines {
        reason = TruncationBy::Bytes;
    }
    result.content = selected.join("\n");
    result.output_lines = selected.len();
    result.output_bytes = result.content.len();
    result.truncated = true;
    result.truncated_by = Some(reason);
    result
}

/// Retain trailing newline-separated entries.
///
/// A final newline terminates the last line without adding an empty entry. An oversized last
/// line keeps a whole-scalar suffix without that newline and sets `last_line_partial`.
#[must_use]
pub fn truncate_tail(content: &str, options: TruncationOptions) -> TruncationResult {
    let mut result = initial_result(content, options);
    if result.total_lines <= result.max_lines && result.total_bytes <= result.max_bytes {
        return result;
    }
    let mut selected = Vec::new();
    let mut bytes = 0;
    let mut reason = TruncationBy::Lines;
    for entry in content.split_terminator('\n').rev().take(result.max_lines) {
        let cost = entry.len() + usize::from(!selected.is_empty());
        if cost > result.max_bytes - bytes {
            reason = TruncationBy::Bytes;
            if selected.is_empty() {
                selected.push(suffix_within_bytes(entry, result.max_bytes));
                result.last_line_partial = true;
            }
            break;
        }
        selected.push(entry);
        bytes += cost;
    }
    if matches!(reason, TruncationBy::Lines) && selected.len() == result.total_lines {
        reason = TruncationBy::Bytes;
    }
    selected.reverse();
    result.content = selected.join("\n");
    result.output_lines = selected.len();
    result.output_bytes = result.content.len();
    result.truncated = true;
    result.truncated_by = Some(reason);
    result
}

/// Select an oversized entry's suffix at the next valid UTF-8 boundary.
fn suffix_within_bytes(entry: &str, max_bytes: usize) -> &str {
    let mut start = entry.len() - max_bytes;
    while !entry.is_char_boundary(start) {
        start += 1;
    }
    &entry[start..]
}

/// A match line's text and clipping flag.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TruncateLineResult {
    /// Original or clipped text.
    pub text: String,
    /// Whether the scalar budget was exceeded.
    pub was_truncated: bool,
}

/// Clip a line on scalar boundaries, appending `... [truncated]` on overflow.
#[must_use]
pub fn truncate_line(line: &str, max_chars: Option<usize>) -> TruncateLineResult {
    let boundary = line
        .char_indices()
        .nth(max_chars.unwrap_or(GREP_MAX_LINE_LENGTH));
    let text = match boundary {
        Some((offset, _)) => format!("{}... [truncated]", &line[..offset]),
        None => line.into(),
    };
    TruncateLineResult {
        text,
        was_truncated: boundary.is_some(),
    }
}

/// Format bytes in B, KB or MB, with one decimal for the latter two units.
#[must_use]
pub fn format_size(bytes: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    if bytes < 1024.0 {
        format!("{}B", buffer.format(bytes))
    } else if bytes < 1024.0 * 1024.0 {
        format!("{}KB", buffer.format_to_fixed(bytes / 1024.0, 1))
    } else {
        format!("{}MB", buffer.format_to_fixed(bytes / (1024.0 * 1024.0), 1))
    }
}
