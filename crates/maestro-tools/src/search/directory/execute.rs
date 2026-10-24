//! Cancellable listing of one directory, and its local-filesystem operations.

use super::{DEFAULT_LIMIT, LsOperations, LsToolDetails, LsToolInput};
use crate::{
    output::render_parts::{float, num},
    path_utils::resolve_existing_path,
    utils::diagnostics::{aborted, blocking, diagnostic, failure, guarded},
};
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use maestro_path::join;
use maestro_theme::Collator;
use std::{path::Path, sync::Arc};

/// The settings every listing shares.
pub(super) struct Lister {
    /// Directory that relative paths are resolved against.
    pub(super) cwd: String,
    /// File access, native or supplied.
    pub(super) operations: Arc<dyn LsOperations>,
}

/// The listing text and details for `entries`, which stopped at `limit` when `reached`.
fn format_entries(entries: &[String], reached: bool, limit: f64) -> AgentToolResult<LsToolDetails> {
    if entries.is_empty() {
        return AgentToolResult::text_result("(empty directory)".into(), None);
    }
    let notices = if reached {
        vec![format!(
            "{} entries limit reached. Use limit={} for more",
            num(limit),
            num(limit * 2.0)
        )]
    } else {
        Vec::new()
    };
    let (output, truncation) =
        super::super::listing::select_listing(&entries.join("\n"), notices, None);
    let details = LsToolDetails {
        truncation,
        entry_limit_reached: reached.then_some(limit),
    };
    let reported = details.truncation.is_some() || details.entry_limit_reached.is_some();
    AgentToolResult::text_result(output, reported.then_some(details))
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
        let directory = resolve_existing_path(requested.unwrap_or("."), &self.cwd)
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
            Ok(maestro_listing::read_dir(absolute_path)?
                .iter()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect())
        }))
    }
}
