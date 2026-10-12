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

/// Guide the model to a command when the first selected line alone exceeds the byte limit.
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
        "{}\n\n[Showing lines {}-{} of {total}{limit_note}. Use offset={} to continue.]",
        truncation.content,
        num(start_display),
        num(end_display),
        num(end_display + 1.0)
    )
}

/// Select the requested lines of `bytes` and describe what remains.
///
/// `path` is the authored path, inserted verbatim, without shell quoting, in oversized-line guidance.
pub(super) fn read_text(
    bytes: &[u8],
    path: &str,
    offset: Option<f64>,
    limit: Option<f64>,
) -> Result<(String, Option<ReadToolDetails>), DiagnosticErrorInfo> {
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.split('\n').collect();
    let total = float(lines.len());
    let start = offset
        .filter(|&o| o != 0.0)
        .map_or(0.0, |o| (o - 1.0).max(0.0));
    if start >= total {
        return Err(failure(&format!(
            "Offset {} is beyond end of file ({} lines total)",
            num(offset.unwrap_or_default()),
            lines.len()
        )));
    }
    let (selected, user_limited) = match limit {
        Some(limit) => {
            let end = (start + limit).min(total);
            (slice(&lines, start, end), Some(end - start))
        }
        None => (slice(&lines, start, total), None),
    };
    let truncation = truncate_head(&selected, TruncationOptions::default());
    let notice = if truncation.first_line_exceeds_limit {
        Some(oversized_line_guidance(&lines, start, path))
    } else if truncation.truncated {
        Some(truncation_notice(&truncation, start + 1.0, lines.len()))
    } else {
        None
    };
    if let Some(text) = notice {
        let details = ReadToolDetails {
            truncation: Some(truncation),
        };
        return Ok((text, Some(details)));
    }
    let text = match user_limited {
        Some(limited) if start + limited < total => format!(
            "{}\n\n[{} more lines in file. Use offset={} to continue.]",
            truncation.content,
            num(total - (start + limited)),
            num(start + limited + 1.0)
        ),
        _ => truncation.content,
    };
    Ok((text, None))
}
