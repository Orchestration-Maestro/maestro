//! Content-search call text and shared listing previews.
use super::super::listing::{ListingView, render_listing};
use crate::output::render_utils::{TruncationView, flag, number};
use crate::{
    output::{
        render_parts::{into_text, limit_label, paint, themed},
        render_utils::{ToolRenderResultLike, invalid_arg_text, render_args, shorten_path, str},
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
    utils::diagnostics::diagnostic,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::interface::ComponentHandle;
use serde_json::Value;
/// Render raw partial arguments, replacing a previous text only after success.
pub(super) fn render_call(
    args: Option<&Value>,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let resolved = themed(theme.get())?;
    let theme = &*resolved;
    let args = render_args(args);
    let invalid = themed(invalid_arg_text(theme))?;
    let pattern = str(args.get("pattern")).map_or_else(
        || Ok(invalid.clone()),
        |p| paint(theme, &ThemeColor::Accent, &format!("/{p}/")),
    )?;
    let path = str(args.get("path"))
        .map(|p| shorten_path(Some(if p.is_empty() { "." } else { p })))
        .transpose()
        .map_err(|e| diagnostic(&e))?
        .unwrap_or(invalid);
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("grep", true))?;
    let mut text = format!(
        "{title} {pattern}{}",
        paint(theme, &ThemeColor::ToolOutput, &format!(" in {path}"))?
    );
    if let Some(glob) = str(args.get("glob")).filter(|g| !g.is_empty()) {
        text += &paint(theme, &ThemeColor::ToolOutput, &format!(" ({glob})"))?;
    }
    if let Some(limit) = args.get("limit") {
        text += &paint(
            theme,
            &ThemeColor::ToolOutput,
            &format!(" limit {}", limit_label(limit)),
        )?;
    }
    Ok(into_text(context.last_component.as_ref(), text))
}
/// Render result options through the shared preview and warning owner.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, Value>,
    options: ToolRenderResultOptions,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let details = result.details;
    let view = ListingView {
        count: number(details, "matchLimitReached"),
        truncation: TruncationView::selected(details),
        noun: "matches",
        preview_lines: 15,
        trailing_warning: flag(details, "linesTruncated").then_some("some lines truncated"),
    };
    let text = render_listing(result, options, theme, context, view)?;
    Ok(into_text(context.last_component.as_ref(), text))
}
