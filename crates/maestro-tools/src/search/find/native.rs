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
use regex_syntax::hir::{
    Class, ClassBytes, ClassBytesRange, ClassUnicode, ClassUnicodeRange, Hir, HirKind, Literal,
};
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

/// The regex fd compiles from `glob` on Windows, read back through `regex-syntax` with every
/// separator it can match widened to `/` or `\`. `None` leaves an invalid glob to fd, which
/// reports it.
fn either_separator(glob: &str) -> Option<String> {
    let glob = globset::GlobBuilder::new(glob)
        .literal_separator(true)
        .backslash_escape(false)
        .build()
        .ok()?;
    let hir = regex_syntax::ParserBuilder::new()
        .utf8(false)
        .dot_matches_new_line(true)
        .build()
        .parse(glob.regex())
        .ok()?;
    let mut regex = String::new();
    regex_syntax::hir::print::Printer::new()
        .print(&widened(hir), &mut regex)
        .ok()?;
    Some(regex)
}

/// `hir` with each literal `/` replaced by a class of both separators, and each class that
/// matches `/` also matching `\`, while one that excludes `/` excludes `\` too.
fn widened(hir: Hir) -> Hir {
    match hir.into_kind() {
        HirKind::Empty => Hir::empty(),
        HirKind::Look(look) => Hir::look(look),
        HirKind::Literal(Literal(bytes)) => {
            let mut parts = Vec::new();
            for (index, part) in bytes.split(|byte| *byte == b'/').enumerate() {
                if index > 0 {
                    parts.push(Hir::class(Class::Bytes(separators())));
                }
                parts.push(Hir::literal(part));
            }
            Hir::concat(parts)
        }
        HirKind::Class(class) => Hir::class(with_both_separators(class)),
        HirKind::Repetition(mut repetition) => {
            repetition.sub = Box::new(widened(*repetition.sub));
            Hir::repetition(repetition)
        }
        HirKind::Capture(mut capture) => {
            capture.sub = Box::new(widened(*capture.sub));
            Hir::capture(capture)
        }
        HirKind::Concat(subs) => Hir::concat(subs.into_iter().map(widened).collect()),
        HirKind::Alternation(subs) => Hir::alternation(subs.into_iter().map(widened).collect()),
    }
}

/// The byte class of both Windows separators.
fn separators() -> ClassBytes {
    ClassBytes::new([
        ClassBytesRange::new(b'/', b'/'),
        ClassBytesRange::new(b'\\', b'\\'),
    ])
}

/// `class` matching `\` exactly when it matches `/`.
fn with_both_separators(mut class: Class) -> Class {
    match &mut class {
        Class::Bytes(set) => {
            let backslash = ClassBytes::new([ClassBytesRange::new(b'\\', b'\\')]);
            if set
                .ranges()
                .iter()
                .any(|range| (range.start()..=range.end()).contains(&b'/'))
            {
                set.union(&backslash);
            } else {
                set.difference(&backslash);
            }
        }
        Class::Unicode(set) => {
            let backslash = ClassUnicode::new([ClassUnicodeRange::new('\\', '\\')]);
            if set
                .ranges()
                .iter()
                .any(|range| (range.start()..=range.end()).contains(&'/'))
            {
                set.union(&backslash);
            } else {
                set.difference(&backslash);
            }
        }
    }
    class
}

#[cfg(test)]
mod tests;
