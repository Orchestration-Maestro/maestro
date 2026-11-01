//! Native search operands, descriptor framing and match-event collection.
use super::GrepToolInput;
use crate::utils::diagnostics::{aborted, diagnostic, failure};
use base64::{Engine, prelude::BASE64_STANDARD};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_process::child_process::{
    CapturedChild, OutputStream, Termination, WaitOptions, wait_for_child_process,
};
use maestro_text::{Utf8Stream, is_whitespace_scalar};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::process::Command;
/// A text or encoded native path in the search protocol.
#[derive(Deserialize)]
#[serde(untagged)]
enum PathRecord {
    /// A UTF-8 path.
    Text {
        /// Decoded text.
        text: String,
    },
    /// Native filename bytes.
    Bytes {
        /// Base64 encoded bytes.
        bytes: String,
    },
}
/// Inline UTF-8 source text.
#[derive(Deserialize)]
struct Lines {
    /// Source line text.
    text: Option<String>,
}
/// Metadata of one matching line.
#[derive(Deserialize, Default)]
struct MatchData {
    /// File identity.
    path: Option<PathRecord>,
    /// One-based line number.
    line_number: Option<usize>,
    /// Inline text when valid UTF-8.
    lines: Option<Lines>,
}
/// The protocol's matching and ignored event kinds.
#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
enum Event {
    /// A matching line.
    #[serde(rename = "match")]
    Match(MatchData),
    /// Non-match metadata.
    #[serde(other)]
    Other,
}
/// Accepted row metadata, retaining native file identity.
pub(super) struct Match {
    /// Native path, not lossy display text.
    pub(super) path: PathBuf,
    /// One-based line number.
    pub(super) line: usize,
    /// Inline match text.
    pub(super) text: Option<String>,
}
/// Incremental stdout records and the count-boundary signal.
#[derive(Default)]
struct Records {
    /// Incomplete final record.
    tail: Vec<u8>,
    /// Accepted matches in producer order.
    rows: Vec<Match>,
    /// Number of match events including incomplete metadata.
    count: usize,
}
impl Records {
    /// Accept one framed event, counting before checking its metadata.
    fn record(&mut self, bytes: &[u8], limit: f64, reached: &Cancellation) {
        if reached.is_aborted() {
            return;
        }
        let Ok(Event::Match(data)) = serde_json::from_slice(bytes) else {
            return;
        };
        self.count += 1;
        if let (Some(path), Some(line)) = (data.path.and_then(path), data.line_number) {
            self.rows.push(Match {
                path,
                line,
                text: data.lines.and_then(|lines| lines.text),
            });
        }
        if crate::output::render_parts::float(self.count) >= limit {
            reached.abort();
        }
    }
    /// Frame complete records without mixing stderr into stdout.
    fn feed(&mut self, bytes: &[u8], limit: f64, reached: &Cancellation) {
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            self.tail.extend_from_slice(part);
            if part.ends_with(b"\n") {
                let record = std::mem::take(&mut self.tail);
                self.record(&record, limit, reached);
            }
        }
    }
}
/// Decode encoded filename identity only at the native path boundary.
fn path(record: PathRecord) -> Option<PathBuf> {
    match record {
        PathRecord::Text { text } => Some(PathBuf::from(text)),
        PathRecord::Bytes { bytes } => {
            let bytes = BASE64_STANDARD.decode(bytes).ok()?;
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStringExt;
                Some(std::ffi::OsString::from_vec(bytes).into())
            }
            #[cfg(not(unix))]
            {
                String::from_utf8(bytes).ok().map(PathBuf::from)
            }
        }
    }
}
/// Build native arguments, keeping user options before the metadata exclusion.
fn command(program: &str, root: &str, input: &GrepToolInput) -> Command {
    let mut command = maestro_process::command(program);
    command.args(["--json", "--line-number", "--color=never", "--hidden"]);
    if input.ignore_case == Some(true) {
        command.arg("--ignore-case");
    }
    if input.literal == Some(true) {
        command.arg("--fixed-strings");
    }
    if let Some(glob) = input.glob.as_ref().filter(|g| !g.is_empty()) {
        command.args(["--glob", glob]);
    }
    command.args([
        "--no-require-git",
        "--glob",
        "!.git",
        "--",
        &input.pattern,
        root,
    ]);
    command
}
/// Observe caller cancellation without a deadline.
async fn cancelled(signal: Option<&Cancellation>) {
    match signal {
        Some(signal) => signal.cancelled().await,
        None => std::future::pending().await,
    }
}
/// Stream the child until exit/EOF, or request ordinary termination at a match limit.
pub(super) async fn search(
    program: &str,
    root: &str,
    input: &GrepToolInput,
    limit: f64,
    signal: Option<&Cancellation>,
) -> Result<(Vec<Match>, usize), DiagnosticErrorInfo> {
    let mut child = CapturedChild::spawn(&mut command(program, root, input))
        .map_err(|e| failure(&format!("Failed to run ripgrep: {e}")))?;
    let mut records = Records::default();
    let reached = Cancellation::new();
    let mut stderr = String::new();
    let mut decoder = Utf8Stream::default();
    let mut sink = |stream, bytes: &[u8]| match stream {
        OutputStream::Stdout => records.feed(bytes, limit, &reached),
        OutputStream::Stderr => stderr.push_str(&decoder.push(bytes)),
    };
    let output = wait_for_child_process(
        &mut child,
        WaitOptions::default(),
        async {
            tokio::select! { () = cancelled(signal) => {}, () = reached.cancelled() => {} }
            Termination::Terminate
        },
        Some(&mut sink),
    )
    .await
    .map_err(|e| diagnostic(&e))?;
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(aborted());
    }
    let tail = std::mem::take(&mut records.tail);
    records.record(&tail, limit, &reached);
    if !reached.is_aborted() && !matches!(output.status.code(), Some(0 | 1)) {
        stderr.push_str(&decoder.finish());
        let error = stderr.trim_matches(is_whitespace_scalar);
        let code = output
            .status
            .code()
            .map_or_else(|| "null".into(), |c| c.to_string());
        return Err(failure(&if error.is_empty() {
            format!("ripgrep exited with code {code}")
        } else {
            error.into()
        }));
    }
    Ok((records.rows, records.count))
}

#[cfg(test)]
mod tests;
