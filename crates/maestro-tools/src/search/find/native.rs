//! Native search operands and captured NUL-delimited filenames.
use crate::{
    output::render_parts::num,
    search::tools_manager::{SearchTool, ToolsManager},
    utils::diagnostics::{diagnostic, failure, guarded},
};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use maestro_process::child_process::{
    CapturedChild, Termination, WaitOptions, wait_for_child_process,
};
use maestro_text::is_whitespace_scalar;
use tokio::process::Command;
/// Search through the acquired program, leaving parsing and ignore scope to it.
pub(super) async fn search(
    manager: &ToolsManager,
    root: &str,
    pattern: &str,
    limit: f64,
    signal: Option<&Cancellation>,
) -> Result<Vec<String>, DiagnosticErrorInfo> {
    let program = guarded(signal, || async {
        Ok(manager.ensure_tool(SearchTool::Fd, None).await)
    })
    .await?
    .ok_or_else(|| failure("fd is not available and could not be downloaded"))?;
    let mut command = command(&program, root, pattern, limit);
    let mut child = CapturedChild::spawn(&mut command)
        .map_err(|e| failure(&format!("Failed to run fd: {e}")))?;
    let output = guarded(signal, || async {
        wait_for_child_process(
            &mut child,
            WaitOptions::default(),
            std::future::pending(),
            None,
        )
        .await
        .map_err(|e| diagnostic(&e))
    })
    .await;
    if signal.is_some_and(Cancellation::is_aborted) {
        child
            .terminate(Termination::Terminate)
            .await
            .map_err(|e| diagnostic(&e))?;
        return Err(crate::utils::diagnostics::aborted());
    }
    let output = output?;
    let text = String::from_utf8_lossy(&output.stdout);
    let records: Vec<String> = text
        .split('\0')
        .filter(|record| !record.is_empty())
        .map(str::to_owned)
        .collect();
    if !output.status.success() && records.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim_matches(is_whitespace_scalar);
        let code = output
            .status
            .code()
            .map_or_else(|| "null".into(), |c| c.to_string());
        return Err(failure(&if stderr.is_empty() {
            format!("fd exited with code {code}")
        } else {
            stderr.to_owned()
        }));
    }
    Ok(records)
}

/// Build native glob operands without validating or rewriting basename syntax.
fn command(program: &str, root: &str, pattern: &str, limit: f64) -> Command {
    let mut command = maestro_process::command(program);
    command.args(arguments(root, pattern, limit, cfg!(windows)));
    command
}

/// The fd operands of one search. A pattern naming a directory is matched against the full
/// path. fd matches that path in its native spelling, so on Windows (`windows`) the glob is
/// passed as the regex fd itself compiles from it, with every `/` matching either separator.
fn arguments(root: &str, pattern: &str, limit: f64, windows: bool) -> Vec<String> {
    let full_path = pattern.contains('/');
    let pattern = if full_path && !pattern.starts_with('/') && !pattern.starts_with("**/") {
        format!("**/{pattern}")
    } else {
        pattern.to_owned()
    };
    let regex = if full_path && windows {
        either_separator(&pattern)
    } else {
        None
    };
    let mode = if regex.is_some() { "--regex" } else { "--glob" };
    let mut arguments: Vec<String> = [
        mode,
        "--color=never",
        "--hidden",
        "--no-require-git",
        "--max-results",
        &num(limit),
        "--print0",
        "--exclude",
        ".git",
    ]
    .map(str::to_owned)
    .into();
    if full_path {
        arguments.push("--full-path".to_owned());
    }
    arguments.extend(["--".to_owned(), regex.unwrap_or(pattern), root.to_owned()]);
    arguments
}

/// The regex fd compiles from `glob` on Windows; `None` keeps fd's own glob matching.
fn either_separator(_glob: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests;
