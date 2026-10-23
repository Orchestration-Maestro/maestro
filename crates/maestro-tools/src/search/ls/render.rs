//! Terminal rendering of ls calls and results.
use crate::output::render_utils::{ToolRenderResultLike, render_args};
use crate::output::render_utils::{TruncationView, number};

use super::super::listing::{ListingView, render_listing};
use crate::{
    output::{
        render_parts::{into_text, limit_label, paint, themed},
        render_utils::{invalid_arg_text, shorten_path, str},
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;

/// Result of a render step.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;

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
    args: Option<&Value>,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let resolved = themed(theme.get())?;
    let theme = &*resolved;
    let args = render_args(args);
    let text = format_ls_call(args, theme)?;
    Ok(into_text(context.last_component.as_ref(), text))
}

/// Draw an ls result, reusing the row's previous `Text`.
///
/// # Errors
/// Returns a theme or escape-expression failure; the previous component is left unchanged.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, Value>,
    options: ToolRenderResultOptions,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let text = render_listing(
        result,
        options,
        theme,
        context,
        ListingView {
            count: number(result.details, "entryLimitReached"),
            truncation: TruncationView::selected(result.details),
            noun: "entries",
            preview_lines: 20,
            trailing_warning: None,
        },
    )?;
    Ok(into_text(context.last_component.as_ref(), text))
}
