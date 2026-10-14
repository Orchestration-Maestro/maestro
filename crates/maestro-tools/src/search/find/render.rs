//! Terminal call text and shared listing previews.
use super::{
    super::listing::{ListingView, render_listing},
    FindToolDetails,
};
use crate::output::render_utils::{ToolRenderResultLike, render_args};
use crate::{
    output::{
        render_parts::{into_text, limit_label, paint, themed},
        render_utils::{invalid_arg_text, shorten_path, str},
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;
/// Render the pattern and directory, retaining a previous text component on success.
pub(super) fn render_call(
    args: Option<&Value>,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let args = render_args(args);
    let invalid = themed(invalid_arg_text(theme))?;
    let pattern = str(args.get("pattern")).map_or_else(
        || Ok(invalid.clone()),
        |p| paint(theme, &ThemeColor::Accent, p),
    )?;
    let path = str(args.get("path"))
        .map(|p| shorten_path(Some(if p.is_empty() { "." } else { p })))
        .transpose()
        .map_err(|e| diagnostic(&e))?
        .unwrap_or(invalid);
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("find", true))?;
    let mut text = format!(
        "{title} {pattern}{}",
        paint(theme, &ThemeColor::ToolOutput, &format!(" in {path}"))?
    );
    if let Some(limit) = args.get("limit") {
        text += &paint(
            theme,
            &ThemeColor::ToolOutput,
            &format!(" (limit {})", limit_label(limit)),
        )?;
    }
    Ok(into_text(context.last_component.as_ref(), text))
}
/// Render a result through the listing preview owner.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, FindToolDetails>,
    options: ToolRenderResultOptions,
    theme: &Theme,
    context: &ToolRenderContext<'_>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let view = ListingView {
        count: result.details.and_then(|d| d.result_limit_reached),
        truncation: result.details.and_then(|d| d.truncation.as_ref()),
        noun: "results",
        preview_lines: 20,
        trailing_warning: None,
    };
    let text = render_listing(result, options, theme, context, view)?;
    Ok(into_text(context.last_component.as_ref(), text))
}
