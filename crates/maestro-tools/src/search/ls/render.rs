//! Terminal rendering of ls calls and results.

use super::LsToolDetails;
use crate::{
    output::{
        render_parts::{expand_hint, float, into_text, num, paint, themed},
        render_utils::{get_text_output, invalid_arg_text, shorten_path, str},
        truncate::format_size,
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_agent::AgentToolResult;
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::{text::utils::is_whitespace_char, tui::ComponentHandle};
use serde_json::Value;

/// Result of a render step.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;

/// Rows of a compact result.
const PREVIEW_LINES: usize = 20;

/// The limit argument as it is spelled in the call line.
fn limit_label(limit: &Value) -> String {
    match limit {
        Value::Number(number) => num(number.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// The call line: the listed path, and the limit when the arguments carry one.
fn format_ls_call(args: &Value, theme: &Theme) -> Rendered<String> {
    let path = str(args.get("path"))
        .map(|raw| shorten_path(Some(if raw.is_empty() { "." } else { raw })))
        .transpose()
        .map_err(|error| diagnostic(&error))?;
    let invalid = themed(invalid_arg_text(theme))?;
    let display = match path {
        None => invalid,
        Some(path) => paint(theme, &ThemeColor::Accent, &path)?,
    };
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("ls", true))?;
    let mut text = format!("{title} {display}");
    if let Some(limit) = args.get("limit") {
        let suffix = format!(" (limit {})", limit_label(limit));
        text += &paint(theme, &ThemeColor::ToolOutput, &suffix)?;
    }
    Ok(text)
}

/// Draw an ls call, reusing the row's previous `Text`.
///
/// # Errors
/// Returns a theme or home failure; the previous component is left unchanged.
pub(super) fn render_call(
    args: &Value,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let text = format_ls_call(args, theme)?;
    Ok(into_text(context.last_component.as_ref(), text))
}

/// The warning line for a listing cut by the entry limit or the byte limit, empty otherwise.
fn limit_warning(details: Option<&LsToolDetails>, theme: &Theme) -> Rendered<String> {
    let entries = details
        .and_then(|details| details.entry_limit_reached)
        .filter(|limit| *limit != 0.0 && !limit.is_nan());
    let bytes = details
        .and_then(|details| details.truncation.as_ref())
        .filter(|cut| cut.truncated);
    if entries.is_none() && bytes.is_none() {
        return Ok(String::new());
    }
    let warnings: Vec<String> = entries
        .map(|limit| format!("{} entries limit", num(limit)))
        .into_iter()
        .chain(bytes.map(|cut| format!("{} limit", format_size(float(cut.max_bytes)))))
        .collect();
    let notice = format!("[Truncated: {}]", warnings.join(", "));
    Ok(format!(
        "\n{}",
        paint(theme, &ThemeColor::Warning, &notice)?
    ))
}

/// The result text: a preview of the listing, an overflow hint and the limit warning.
fn format_ls_result(
    result: &AgentToolResult<LsToolDetails>,
    options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<String> {
    let output = get_text_output(Some(result), context.show_images, &context.terminal_image)?;
    let output =
        output.trim_matches(|scalar: char| is_whitespace_char(scalar.encode_utf8(&mut [0; 4])));
    let mut text = String::new();
    if !output.is_empty() {
        let lines: Vec<&str> = output.split('\n').collect();
        let shown = if options.expanded {
            lines.len()
        } else {
            PREVIEW_LINES.min(lines.len())
        };
        let rows = lines[..shown]
            .iter()
            .map(|line| paint(theme, &ThemeColor::ToolOutput, line))
            .collect::<Rendered<Vec<_>>>()?;
        text = ["\n", &rows.join("\n")].concat();
        if lines.len() > shown {
            let counts = format!("{} more lines", lines.len() - shown);
            text += &expand_hint(&counts, theme, &context.live_theme)?;
        }
    }
    Ok(text + &limit_warning(result.details.as_ref(), theme)?)
}

/// Draw an ls result, reusing the row's previous `Text`.
///
/// # Errors
/// Returns a theme or escape-expression failure; the previous component is left unchanged.
pub(super) fn render_result(
    result: &AgentToolResult<LsToolDetails>,
    options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let text = format_ls_result(result, options, theme, context)?;
    Ok(into_text(context.last_component.as_ref(), text))
}
