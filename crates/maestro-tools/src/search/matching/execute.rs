//! File effect routing, cached context and output selection.
use super::{GrepOperations, GrepToolDetails, GrepToolInput, native::Match};
use crate::{
    output::{
        limits::truncate_line,
        render_parts::{float, num},
    },
    path_utils::{resolve_existing_path, resolve_path},
    search::{
        acquisition::{SearchTool, ToolsManager},
        listing::select_listing,
    },
    utils::diagnostics::{aborted, blocking, diagnostic, failure, guarded},
};
use maestro_agent::AgentToolResult;
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_path::{Cwd, SEP, basename, relative};
use maestro_text::normalize_to_lf;
use num_traits::ToPrimitive;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
/// Configuration captured by the tool definition.
pub(super) struct Searcher {
    /// Base for authored paths.
    pub(super) cwd: String,
    /// Selected replacement file effects.
    pub(super) operations: Option<Arc<dyn GrepOperations>>,
    /// Native search-program owner.
    pub(super) tools_manager: Arc<ToolsManager>,
}
impl Searcher {
    /// Inspect the root using the selected adapter.
    async fn is_directory(&self, root: &str) -> Result<bool, DiagnosticErrorInfo> {
        if let Some(operations) = &self.operations {
            return operations.is_directory(root.into()).await;
        }
        let path = PathBuf::from(root);
        blocking(move || std::fs::metadata(path).map(|m| m.is_dir())).await
    }
    /// Read context without turning a lossy filename into native identity.
    async fn read(&self, path: PathBuf) -> Result<String, DiagnosticErrorInfo> {
        match &self.operations {
            Some(operations) => {
                operations
                    .read_file(path.to_string_lossy().into_owned())
                    .await
            }
            None => {
                blocking(move || {
                    std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
                })
                .await
            }
        }
    }
    /// Acquire first, then resolve and inspect the root before native execution.
    pub(super) async fn search(
        &self,
        input: GrepToolInput,
        signal: Option<Cancellation>,
    ) -> Result<AgentToolResult<GrepToolDetails>, DiagnosticErrorInfo> {
        let signal = signal.as_ref();
        let program = guarded(signal, || async {
            Ok(self.tools_manager.ensure_tool(SearchTool::Rg, None).await)
        })
        .await?
        .ok_or_else(|| failure("ripgrep (rg) is not available and could not be downloaded"))?;
        let root = resolve_existing_path(
            input
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("."),
            &self.cwd,
        )
        .map_err(|e| diagnostic(&e))?;
        let directory = guarded(signal, || self.is_directory(&root))
            .await
            .map_err(|e| {
                if signal.is_some_and(Cancellation::is_aborted) {
                    e
                } else {
                    failure(&format!("Path not found: {root}"))
                }
            })?;
        let limit = input.limit.unwrap_or(100.0).max(1.0);
        let (matches, count) =
            super::native::search(&program, &root, &input, limit, signal).await?;
        if count == 0 {
            return Ok(AgentToolResult::text_result(
                "No matches found".into(),
                None,
            ));
        }
        let mut body = Body::default();
        let context = input.context.unwrap_or(0.0).max(0.0).floor();
        for row in matches {
            guarded(signal, || {
                body.append(self, row, (&root, directory, context))
            })
            .await?;
        }
        if signal.is_some_and(Cancellation::is_aborted) {
            return Err(aborted());
        }
        Ok(body.finish((float(count) >= limit).then_some(limit)))
    }
}
/// Context cache and displayed rows for one invocation.
#[derive(Default)]
struct Body {
    /// Cache includes unreadable files and uses native identity.
    cache: HashMap<PathBuf, Option<Vec<String>>>,
    /// Rows in match-event order.
    rows: Vec<String>,
    /// Whether any line was clipped.
    clipped: bool,
}
impl Body {
    /// Append inline text or a cached context block, preserving overlapping rows.
    async fn append(
        &mut self,
        searcher: &Searcher,
        row: Match,
        scope: (&str, bool, f64),
    ) -> Result<(), DiagnosticErrorInfo> {
        let (root, directory, context) = scope;
        let name = display_path(root, &row.path, directory)?;
        if let Some(text) = row.text.filter(|_| context == 0.0) {
            let text = text.replace("\r\n", "\n").replace('\r', "");
            self.clipped |= append_line(
                &mut self.rows,
                &name,
                row.line,
                ':',
                text.strip_suffix('\n').unwrap_or(&text),
            );
            return Ok(());
        }
        let path = if row.path.is_absolute() {
            row.path
        } else {
            PathBuf::from(root).join(row.path)
        };
        if !self.cache.contains_key(&path) {
            let text = searcher.read(path.clone()).await.ok();
            self.cache.insert(
                path.clone(),
                text.map(|text| {
                    normalize_to_lf(&text)
                        .split('\n')
                        .map(str::to_owned)
                        .collect()
                }),
            );
        }
        let Some(lines) = self.cache.get(&path).and_then(Option::as_ref) else {
            self.rows
                .push(format!("{name}:{}: (unable to read file)", row.line));
            return Ok(());
        };
        let start = (float(row.line) - context - 1.0)
            .max(0.0)
            .min(float(lines.len()))
            .to_usize()
            .unwrap_or(lines.len());
        let end = (float(row.line) + context)
            .min(float(lines.len()))
            .to_usize()
            .unwrap_or(lines.len());
        for (index, text) in lines
            .iter()
            .enumerate()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            let line = index + 1;
            let separator = if line == row.line { ':' } else { '-' };
            self.clipped |= append_line(&mut self.rows, &name, line, separator, text);
        }
        Ok(())
    }
    /// Apply count, byte and scalar notices in that order.
    fn finish(self, limit: Option<f64>) -> AgentToolResult<GrepToolDetails> {
        let notices = limit
            .map(|limit| {
                format!(
                    "{} matches limit reached. Use limit={} for more, or refine pattern",
                    num(limit),
                    num(limit * 2.0)
                )
            })
            .into_iter()
            .collect();
        let trailing = self
            .clipped
            .then_some("Some lines truncated to 500 chars. Use read tool to see full lines");
        let (text, truncation) = select_listing(&self.rows.join("\n"), notices, trailing);
        let details =
            (limit.is_some() || truncation.is_some() || self.clipped).then_some(GrepToolDetails {
                truncation,
                match_limit_reached: limit,
                lines_truncated: self.clipped.then_some(true),
            });
        AgentToolResult::text_result(text, details)
    }
}
/// Project display text against a lexical root, falling back only for actual parent components.
fn display_path(
    root: &str,
    path: &std::path::Path,
    directory: bool,
) -> Result<String, DiagnosticErrorInfo> {
    let text = path.to_string_lossy();
    let absolute = resolve_path(&[root, &text]).map_err(|e| diagnostic(&e))?;
    let relative = relative(
        root,
        &absolute,
        &Cwd {
            current: root,
            drive_directories: &[],
        },
    );
    let display = if directory && relative != ".." && !relative.starts_with(&format!("..{SEP}")) {
        relative
    } else {
        basename(&text, None)
    };
    Ok(display.replace(SEP, "/"))
}

/// Append one source row through the shared scalar clipping owner.
fn append_line(
    rows: &mut Vec<String>,
    name: &str,
    line: usize,
    separator: char,
    text: &str,
) -> bool {
    let cut = truncate_line(text, None);
    rows.push(format!("{name}{separator}{line}{separator} {}", cut.text));
    cut.was_truncated
}
