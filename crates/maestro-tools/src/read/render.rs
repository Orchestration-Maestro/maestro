//! Terminal rendering of read calls and results.

use super::ReadToolDetails;
use crate::{
    keybinding_hints::{EXPAND, key_text},
    output::{
        render_parts::{expand_hint, float, into_text, paint},
        render_utils::{
            PREVIEW_LINES, format_read_line_range, get_text_output, path_display, replace_tabs,
            selected_path, str, without_trailing_empty,
        },
        truncate::{TruncationBy, format_size},
    },
    path_utils::resolve_read_path,
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_agent::AgentToolResult;
use maestro_models::DiagnosticErrorInfo;
use maestro_path::{Cwd, SEP, basename, dirname, relative, try_resolve};
use maestro_theme::{Theme, ThemeColor, get_language_from_path, highlight_code};
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;

/// Result of a render step.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;

/// Names that mark a resource file.
const RESOURCE_FILE_NAMES: [&str; 4] = ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"];

/// What a compact read is shown as.
enum Kind {
    /// A documentation file.
    Docs,
    /// A resource file.
    Resource,
    /// A skill file.
    Skill,
}

/// A compact read's kind and label.
struct Classification {
    /// Kind of file.
    kind: Kind,
    /// Name shown for it.
    label: String,
}

/// The shared range suffix in the read warning color.
fn line_range(args: &Value, theme: &Theme) -> Rendered<String> {
    let range = format_read_line_range(args);
    if range.is_empty() {
        return Ok(range);
    }
    paint(theme, &ThemeColor::Warning, &range)
}

/// The call line of an unclassified read.
fn format_read_call(args: &Value, theme: &Theme) -> Rendered<String> {
    let display = path_display(selected_path(args), theme)?;
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("read", true))?;
    Ok(format!("{title} {display}{}", line_range(args, theme)?))
}

/// Documentation under the README's directory, labelled by its relative path.
fn docs_classification(
    absolute: &str,
    context: &ToolRenderContext<'_>,
) -> Rendered<Option<Classification>> {
    let readme_dir = dirname(&context.readme_path);
    let process_cwd;
    let anchored =
        try_resolve(&[&readme_dir], &[]).is_ok() && try_resolve(&[absolute], &[]).is_ok();
    let cwd = if anchored {
        Cwd {
            current: "",
            drive_directories: &[],
        }
    } else {
        process_cwd = std::env::current_dir()
            .map_err(|error| diagnostic(&error))?
            .to_string_lossy()
            .into_owned();
        Cwd {
            current: &process_cwd,
            drive_directories: &[],
        }
    };
    let path = relative(&readme_dir, absolute, &cwd);
    let separator = SEP.to_string();
    let outside = path.is_empty()
        || path == ".."
        || path.starts_with(&format!("..{SEP}"))
        || maestro_path::is_absolute(&path);
    let label = path.replace(&separator, "/");
    let docs = label == "README.md" || label.starts_with("docs/") || label.starts_with("examples/");
    Ok((!outside && docs).then_some(Classification {
        kind: Kind::Docs,
        label,
    }))
}

/// Skill, documentation or resource files are shown compactly.
fn classify(args: &Value, context: &ToolRenderContext<'_>) -> Rendered<Option<Classification>> {
    let Some(raw) = str(selected_path(args)).filter(|raw| !raw.is_empty()) else {
        return Ok(None);
    };
    let absolute = resolve_read_path(raw, &context.cwd).map_err(|error| diagnostic(&error))?;
    let name = basename(&absolute, None);
    if name == "SKILL.md" {
        let parent = basename(&dirname(&absolute), None);
        let label = if parent.is_empty() { name } else { parent };
        return Ok(Some(Classification {
            kind: Kind::Skill,
            label,
        }));
    }
    Ok(docs_classification(&absolute, context)?.or_else(|| {
        RESOURCE_FILE_NAMES
            .contains(&name.as_str())
            .then_some(Classification {
                kind: Kind::Resource,
                label: name,
            })
    }))
}

/// The call line of a compact read.
fn format_compact_call(
    classification: &Classification,
    args: &Value,
    theme: &Theme,
) -> Rendered<String> {
    let hint = format!(" ({} to expand)", key_text(EXPAND));
    let hint = paint(theme, &ThemeColor::Dim, &hint)?;
    let range = |before: String| Ok(before + &line_range(args, theme)? + &hint);
    let label = &classification.label;
    let kind = match classification.kind {
        Kind::Skill => {
            let marker = paint(
                theme,
                &ThemeColor::CustomMessageLabel,
                "\x1b[1m[skill]\x1b[22m ",
            )?;
            return range(marker + &paint(theme, &ThemeColor::CustomMessageText, label)?);
        }
        Kind::Docs => "docs",
        Kind::Resource => "resource",
    };
    let title = paint(
        theme,
        &ThemeColor::ToolTitle,
        &Theme::bold(&format!("read {kind}"), true),
    )?;
    range(format!(
        "{title} {}",
        paint(theme, &ThemeColor::Accent, label)?
    ))
}

/// Draw a read call, reusing the row's previous `Text`.
///
/// # Errors
/// Returns a theme, home or path failure; the previous component is left unchanged.
pub(super) fn render_call(
    args: &Value,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let classification = if context.expanded {
        None
    } else {
        classify(args, context)?
    };
    let text = match classification {
        Some(classification) => format_compact_call(&classification, args, theme)?,
        None => format_read_call(args, theme)?,
    };
    Ok(into_text(context.last_component.as_ref(), text))
}

/// The warning line for a truncated read, empty when nothing was cut.
fn truncation_notice(details: Option<&ReadToolDetails>, theme: &Theme) -> Rendered<String> {
    let Some(cut) = details
        .and_then(|details| details.truncation.as_ref())
        .filter(|cut| cut.truncated)
    else {
        return Ok(String::new());
    };
    let notice = if cut.first_line_exceeds_limit {
        format!(
            "[First line exceeds {} limit]",
            format_size(float(cut.max_bytes))
        )
    } else if matches!(cut.truncated_by, Some(TruncationBy::Lines)) {
        format!(
            "[Truncated: showing {} of {} lines ({} line limit)]",
            cut.output_lines, cut.total_lines, cut.max_lines
        )
    } else {
        format!(
            "[Truncated: {} lines shown ({} limit)]",
            cut.output_lines,
            format_size(float(cut.max_bytes))
        )
    };
    Ok(format!(
        "\n{}",
        paint(theme, &ThemeColor::Warning, &notice)?
    ))
}

/// The result text: a preview of the output, an overflow hint and the truncation notice.
fn format_read_result(
    result: &AgentToolResult<ReadToolDetails>,
    options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<String> {
    let output = get_text_output(Some(result), context.show_images, &context.terminal_image)?;
    let language = str(selected_path(context.args))
        .filter(|raw| !raw.is_empty())
        .and_then(get_language_from_path);
    let lines = match language {
        Some(language) => highlight_code(
            &context.live_theme,
            &*context.highlighter,
            &replace_tabs(&output),
            Some(language),
        ),
        None => output.split('\n').map(str::to_owned).collect(),
    };
    let lines = without_trailing_empty(&lines);
    let shown = if options.expanded {
        lines.len()
    } else {
        PREVIEW_LINES.min(lines.len())
    };
    let rows = lines[..shown]
        .iter()
        .map(|line| match language {
            Some(_) => Ok(replace_tabs(line)),
            None => paint(theme, &ThemeColor::ToolOutput, &replace_tabs(line)),
        })
        .collect::<Rendered<Vec<_>>>()?;
    let mut text = ["\n", &rows.join("\n")].concat();
    if lines.len() > shown {
        let counts = format!("{} more lines", lines.len() - shown);
        text += &expand_hint(&counts, theme, &context.live_theme)?;
    }
    Ok(text + &truncation_notice(result.details.as_ref(), theme)?)
}

/// Draw a read result, reusing the row's previous `Text`.
///
/// # Errors
/// Returns a theme, home, path or escape-expression failure; the previous component is left
/// unchanged.
pub(super) fn render_result(
    result: &AgentToolResult<ReadToolDetails>,
    options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let hidden =
        !options.expanded && !context.is_error && classify(context.args, context)?.is_some();
    let text = if hidden {
        String::new()
    } else {
        format_read_result(result, options, theme, context)?
    };
    Ok(into_text(context.last_component.as_ref(), text))
}
