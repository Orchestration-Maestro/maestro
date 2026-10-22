//! Search routing, path projection and result policy.
use super::{FindGlobOptions, FindOperations, FindToolDetails, FindToolInput};
use crate::{
    output::render_parts::{float, num},
    path_utils::{resolve_existing_path, resolve_path},
    search::{listing::select_listing, tools_manager::ToolsManager},
    utils::diagnostics::{aborted, diagnostic, failure, guarded},
};
use maestro_agent::AgentToolResult;
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_path::{Cwd, SEP, relative};
use std::sync::Arc;
/// Captured configuration of a search definition.
pub(super) struct Finder {
    /// Base for authored relative directories.
    pub(super) cwd: String,
    /// Supplied file effects, if selected.
    pub(super) operations: Option<Arc<dyn FindOperations>>,
    /// Native program discovery owner.
    pub(super) tools_manager: Arc<ToolsManager>,
}
/// Project an already-valued path against its search root without authored expansion.
fn relative_path(root: &str, path: &str) -> Result<String, DiagnosticErrorInfo> {
    let absolute = resolve_path(&[root, path]).map_err(|e| diagnostic(&e))?;
    let cwd = Cwd {
        current: root,
        drive_directories: &[],
    };
    let mut result = relative(root, &absolute, &cwd).replace(SEP, "/");
    if (path.ends_with(SEP) || (cfg!(windows) && path.ends_with('/'))) && !result.ends_with('/') {
        result.push('/');
    }
    Ok(result)
}
impl Finder {
    /// Run one search, preserving the producer's ordering and duplicates.
    pub(super) async fn find(
        &self,
        input: FindToolInput,
        signal: Option<Cancellation>,
    ) -> Result<AgentToolResult<FindToolDetails>, DiagnosticErrorInfo> {
        let signal = signal.as_ref();
        if signal.is_some_and(Cancellation::is_aborted) {
            return Err(aborted());
        }
        let root = resolve_existing_path(
            input
                .path
                .as_deref()
                .filter(|p| !p.is_empty())
                .unwrap_or("."),
            &self.cwd,
        )
        .map_err(|e| diagnostic(&e))?;
        let limit = input.limit.unwrap_or(1000.0);
        let native = self.operations.is_none();
        let paths = if let Some(operations) = &self.operations {
            if !guarded(signal, || operations.exists(root.clone())).await? {
                return Err(failure(&format!("Path not found: {root}")));
            }
            guarded(signal, || {
                operations.glob(
                    input.pattern,
                    root.clone(),
                    FindGlobOptions {
                        ignore: vec!["**/node_modules/**".into(), "**/.git/**".into()],
                        limit,
                    },
                )
            })
            .await?
        } else {
            super::native::search(&self.tools_manager, &root, &input.pattern, limit, signal).await?
        };
        format_paths(&paths, &root, limit, native)
    }
}

/// Join projected paths and report this route's count and byte limits.
fn format_paths(
    paths: &[String],
    root: &str,
    limit: f64,
    native: bool,
) -> Result<AgentToolResult<FindToolDetails>, DiagnosticErrorInfo> {
    if paths.is_empty() {
        return Ok(AgentToolResult::text_result(
            "No files found matching pattern".into(),
            None,
        ));
    }
    let root = resolve_path(&[root]).map_err(|e| diagnostic(&e))?;
    let reached = (!native || limit > 0.0) && float(paths.len()) >= limit;
    let body = paths
        .iter()
        .map(|path| relative_path(&root, path))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    let notices = if reached {
        let advice = if native {
            format!(
                ". Use limit={} for more, or refine pattern",
                num(limit * 2.0)
            )
        } else {
            String::new()
        };
        vec![format!("{} results limit reached{advice}", num(limit))]
    } else {
        Vec::new()
    };
    let (text, truncation) = select_listing(&body, notices, None);
    let reported = reached || truncation.is_some();
    Ok(AgentToolResult::text_result(
        text,
        reported.then_some(FindToolDetails {
            truncation,
            result_limit_reached: reached.then_some(limit),
        }),
    ))
}
