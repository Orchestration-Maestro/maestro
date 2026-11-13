//! Line selection and continuation notices for text reads.

use super::ReadToolDetails;
use crate::{
    output::{
        render_parts::{float, num},
        truncate::{
            DEFAULT_MAX_BYTES, TruncationBy, TruncationOptions, TruncationResult, format_size,
            truncate_head,
        },
    },
    utils::diagnostics::failure,
};
use maestro_models::DiagnosticErrorInfo;
use num_traits::ToPrimitive;

/// Resolve a slice bound: truncate toward zero, count a negative bound from the end.
fn bound(value: f64, len: usize) -> usize {
    let whole = value.trunc();
    let index = if whole < 0.0 {
        (float(len) + whole).max(0.0)
    } else {
        whole.min(float(len))
    };
    index.to_usize().unwrap_or(len)
}

/// Join the entries between two numeric slice bounds.
fn slice(lines: &[&str], start: f64, end: f64) -> String {
    let (from, to) = (bound(start, lines.len()), bound(end, lines.len()));
    lines.get(from..to).unwrap_or_default().join("\n")
}

/// Tell the model which command to run when even the first selected line is larger than the byte limit.
fn oversized_line_guidance(lines: &[&str], start: f64, path: &str) -> String {
    let line = bound(start, lines.len());
    let size = format_size(float(lines.get(line).map_or(0, |l| l.len())));
    let shown = num(float(line) + 1.0);
    let max = format_size(float(DEFAULT_MAX_BYTES));
    format!(
        "[Line {shown} is {size}, exceeds {max} limit. Use bash: sed -n '{shown}p' {path} | head -c {DEFAULT_MAX_BYTES}]"
    )
}

/// Append the continuation notice for output cut by the line or byte limit.
fn truncation_notice(truncation: &TruncationResult, start_display: f64, total: usize) -> String {
    let end_display = start_display + float(truncation.output_lines) - 1.0;
    let limit_note = match truncation.truncated_by {
        Some(TruncationBy::Bytes) => format!(" ({} limit)", format_size(float(DEFAULT_MAX_BYTES))),
        Some(TruncationBy::Lines) | None => String::new(),
    };
    format!(
        "{}\n\n[Showing lines {}-{} of {total}{limit_note}. Continue with offset={}.]",
        truncation.content,
        num(start_display),
        num(end_display),
        num(end_display + 1.0)
    )
}

/// Select the requested lines of `bytes` and describe what remains.
///
/// Lines are separated by LF, and a terminating newline ends the last line instead of starting an
/// empty one. `path` is the authored path, inserted verbatim, without shell quoting, in
/// oversized-line guidance.
pub(super) fn read_text(
    bytes: &[u8],
    path: &str,
    offset: Option<f64>,
    limit: Option<f64>,
) -> Result<(String, Option<ReadToolDetails>), DiagnosticErrorInfo> {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.split('\n').collect();
    let line_count = lines.len() - usize::from(text.ends_with('\n'));
    let total = float(line_count);
    let start = offset
        .filter(|&o| o != 0.0)
        .map_or(0.0, |o| (o - 1.0).max(0.0));
    if start >= total {
        return Err(failure(&format!(
            "Offset {} is beyond end of file ({line_count} lines total)",
            num(offset.unwrap_or_default())
        )));
    }
    let end = limit.map_or(total, |limit| (start + limit).min(total));
    // A selection reaching the end keeps the final newline the split left as its last entry.
    let shown_end = if end >= total {
        float(lines.len())
    } else {
        end
    };
    let selected = slice(&lines, start, shown_end);
    let truncation = truncate_head(&selected, TruncationOptions::default());
    let notice = if truncation.first_line_exceeds_limit {
        Some(oversized_line_guidance(&lines, start, path))
    } else if truncation.truncated && start + float(truncation.output_lines) < end {
        // Every selected line is shown: only the terminating newline passed the limit.
        Some(truncation_notice(&truncation, start + 1.0, line_count))
    } else {
        None
    };
    if let Some(text) = notice {
        let details = ReadToolDetails {
            truncation: Some(truncation),
        };
        return Ok((text, Some(details)));
    }
    let text = if limit.is_some() && end < total {
        format!(
            "{}\n\n[{} more lines remain in the file. Continue with offset={}.]",
            truncation.content,
            num(total - end),
            num(end + 1.0)
        )
    } else {
        truncation.content
    };
    Ok((text, None))
}
