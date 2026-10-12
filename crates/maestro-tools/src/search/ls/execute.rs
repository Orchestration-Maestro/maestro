//! Cancellable listing of one directory, and its local-filesystem operations.

use super::{DEFAULT_LIMIT, LsOperations, LsToolDetails, LsToolInput};
use crate::{
    output::{
        render_parts::{float, num},
        truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head},
    },
    path_utils::resolve_to_cwd,
    utils::diagnostics::{aborted, blocking, diagnostic, failure, guarded},
};
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, TextContent, UserBlock};
use maestro_path::join;
use maestro_theme::{Collator, NativeThemeOperations, ThemeOperations};
use std::{path::Path, sync::Arc};

/// The largest integer a double holds exactly, which leaves the line budget unbounded.
const UNBOUNDED_LINES: u64 = 9_007_199_254_740_991;

/// The settings every listing shares.
pub(super) struct Lister {
    /// Directory that relative paths are resolved against.
    pub(super) cwd: String,
    /// File access, native or supplied.
    pub(super) operations: Arc<dyn LsOperations>,
}

/// A result of one text block.
fn text_result(text: String, details: Option<LsToolDetails>) -> AgentToolResult<LsToolDetails> {
    AgentToolResult {
        content: vec![UserBlock::Text(TextContent {
            text,
            text_signature: None,
        })],
        details,
        terminate: None,
    }
}

/// The listing text and details for `entries`, which stopped at `limit` when `reached`.
fn format_entries(entries: &[String], reached: bool, limit: f64) -> AgentToolResult<LsToolDetails> {
    if entries.is_empty() {
        return text_result("(empty directory)".into(), None);
    }
    let truncation = truncate_head(
        &entries.join("\n"),
        TruncationOptions {
            max_lines: Some(usize::try_from(UNBOUNDED_LINES).unwrap_or(usize::MAX)),
            max_bytes: None,
        },
    );
    let mut output = truncation.content.clone();
    let mut notices = Vec::new();
    if reached {
        notices.push(format!(
            "{} entries limit reached. Use limit={} for more",
            num(limit),
            num(limit * 2.0)
        ));
    }
    if truncation.truncated {
        notices.push(format!(
            "{} limit reached",
            format_size(float(DEFAULT_MAX_BYTES))
        ));
    }
    if !notices.is_empty() {
        output = format!("{output}\n\n[{}]", notices.join(". "));
    }
    let details = LsToolDetails {
        truncation: truncation.truncated.then_some(truncation),
        entry_limit_reached: reached.then_some(limit),
    };
    let reported = details.truncation.is_some() || details.entry_limit_reached.is_some();
    text_result(output, reported.then_some(details))
}

impl Lister {
    /// List the directory named by `input`, honouring `signal` before every operation.
    pub(super) async fn list(
        &self,
        input: LsToolInput,
        signal: Option<Cancellation>,
    ) -> Result<AgentToolResult<LsToolDetails>, DiagnosticErrorInfo> {
        let signal = signal.as_ref();
        let cancelled = || signal.is_some_and(Cancellation::is_aborted);
        if cancelled() {
            return Err(aborted());
        }
        let requested = input.path.as_deref().filter(|path| !path.is_empty());
        let directory = resolve_to_cwd(requested.unwrap_or("."), &self.cwd)
            .map_err(|error| diagnostic(&error))?;
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
        let operations = &self.operations;
        if !guarded(signal, || operations.exists(directory.clone())).await? {
            return Err(failure(&format!("Path not found: {directory}")));
        }
        if !guarded(signal, || operations.stat(directory.clone())).await? {
            return Err(failure(&format!("Not a directory: {directory}")));
        }
        let names = guarded(signal, || operations.readdir(directory.clone()))
            .await
            .map_err(|error| {
                if cancelled() {
                    error
                } else {
                    failure(&format!("Cannot read directory: {}", error.message))
                }
            })?;
        let collator =
            Collator::new(&|name| std::env::var(name).ok()).map_err(|error| diagnostic(&error))?;
        let mut keyed: Vec<(String, String)> = names
            .into_iter()
            .map(|name| (name.to_lowercase(), name))
            .collect();
        keyed.sort_by(|left, right| collator.compare(&left.0, &right.0));
        let mut entries = Vec::new();
        let mut reached = false;
        for (_, name) in keyed {
            if float(entries.len()) >= limit {
                reached = true;
                break;
            }
            let path = join(&[&directory, &name]);
            match guarded(signal, || operations.stat(path)).await {
                Ok(true) => entries.push(name + "/"),
                Ok(false) => entries.push(name),
                Err(error) if cancelled() => return Err(error),
                Err(_) => {}
            }
        }
        Ok(format_entries(&entries, reached, limit))
    }
}

/// The default operations: examine and list the local filesystem.
pub(super) struct NativeLsOperations;

impl LsOperations for NativeLsOperations {
    fn exists(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        Box::pin(blocking(move || Ok(Path::new(&absolute_path).exists())))
    }

    fn stat(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>> {
        Box::pin(blocking(move || {
            std::fs::metadata(absolute_path).map(|metadata| metadata.is_dir())
        }))
    }

    fn readdir(
        &self,
        absolute_path: String,
    ) -> BoxFuture<Result<Vec<String>, DiagnosticErrorInfo>> {
        Box::pin(blocking(move || {
            NativeThemeOperations.read_dir(&absolute_path)
        }))
    }
}
