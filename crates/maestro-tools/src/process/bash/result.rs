//! Final tail notices and shell status results.
use super::{BashError, BashExecResult, BashToolDetails};
use crate::{
    TruncationBy, format_size,
    output::{
        output_accumulator::OutputSnapshot,
        render_parts::{float, num},
    },
    utils::diagnostics::{diagnostic, failure},
};
use maestro_agent::AgentToolResult;
use maestro_models::DiagnosticErrorInfo;

/// Byte counts of the two lines a partial tail notice can name.
#[derive(Clone, Copy)]
pub(super) struct LineBytes {
    /// The unterminated line at the end of the output.
    pub(super) last: usize,
    /// The most recent newline-terminated line.
    pub(super) ended: usize,
}

/// Format the selected tail, preserving raw text and its newline entries.
fn format(
    snapshot: OutputSnapshot,
    line_bytes: LineBytes,
    empty: &str,
) -> (String, Option<BashToolDetails>) {
    let OutputSnapshot {
        content,
        truncation,
        full_output_path,
    } = snapshot;
    let mut text = if content.is_empty() {
        empty.into()
    } else {
        content
    };
    if !truncation.truncated {
        return (text, None);
    }
    let start = truncation.total_lines - truncation.output_lines + 1;
    let end = truncation.total_lines;
    let path = full_output_path.as_deref().unwrap_or_default();
    let notice = if truncation.last_line_partial {
        // A final newline ends the partial line, leaving an empty entry after it.
        let (line, bytes) = if text.ends_with('\n') {
            (end - 1, line_bytes.ended)
        } else {
            (end, line_bytes.last)
        };
        format!(
            "[Showing last {} of line {line} (line is {}). Full output: {path}]",
            format_size(float(truncation.output_bytes)),
            format_size(float(bytes))
        )
    } else if matches!(truncation.truncated_by, Some(TruncationBy::Lines)) {
        format!("[Showing lines {start}-{end} of {end}. Full output: {path}]")
    } else {
        format!(
            "[Showing lines {start}-{end} of {end} ({} limit). Full output: {path}]",
            format_size(float(truncation.max_bytes))
        )
    };
    text.push_str("\n\n");
    text.push_str(&notice);
    (
        text,
        Some(BashToolDetails {
            truncation: Some(truncation),
            full_output_path,
        }),
    )
}

/// Append a status only after nonempty output.
fn status(text: &str, message: &str) -> DiagnosticErrorInfo {
    failure(&if text.is_empty() {
        message.into()
    } else {
        format!("{text}\n\n{message}")
    })
}

/// Classify settlement after decoding and artifact closure.
pub(super) fn settle(
    snapshot: OutputSnapshot,
    line_bytes: LineBytes,
    result: Result<BashExecResult, BashError>,
) -> Result<AgentToolResult<BashToolDetails>, DiagnosticErrorInfo> {
    let (text, details) = format(
        snapshot,
        line_bytes,
        if result.is_ok() { "(no output)" } else { "" },
    );
    match result {
        Ok(result) => {
            if let Some(code) = result.exit_code.filter(|code| *code != 0.0) {
                return Err(status(
                    &text,
                    &format!("Command exited with code {}", num(code)),
                ));
            }
            Ok(AgentToolResult::text_result(text, details))
        }
        Err(BashError::Aborted) => Err(status(&text, "Command aborted")),
        Err(BashError::Timeout(seconds)) => Err(status(
            &text,
            &format!("Command timed out after {} seconds", num(seconds)),
        )),
        Err(BashError::Other(error)) => {
            let message = error.to_string();
            if message == "aborted" {
                return Err(status(&text, "Command aborted"));
            }
            if let Some(timeout) = message.strip_prefix("timeout:") {
                let seconds = timeout.split(':').next().unwrap_or_default();
                return Err(status(
                    &text,
                    &format!("Command timed out after {seconds} seconds"),
                ));
            }
            Err(error
                .downcast_ref::<std::io::Error>()
                .map_or_else(|| failure(&message), diagnostic))
        }
    }
}
