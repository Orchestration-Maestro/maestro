//! Listing text selection, notices and terminal previews shared by file tools.
use crate::output::render_utils::ToolRenderResultLike;
use crate::{
    output::{
        render_parts::{expand_hint, float, num, paint, themed},
        render_utils::{TruncationView, get_text_output},
        truncate::{
            DEFAULT_MAX_BYTES, TruncationOptions, TruncationResult, format_size, truncate_head,
        },
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
};
use maestro_models::DiagnosticErrorInfo;
use maestro_text::is_whitespace_scalar;
use maestro_theme::{LiveTheme, ThemeColor};
/// Result of a rendering operation.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;
/// Borrowed limits of a listing result.
#[derive(Clone, Copy)]
pub(super) struct ListingView<'a> {
    /// Reported count, if any.
    pub(super) count: Option<f64>,
    /// Reported byte selection, if any.
    pub(super) truncation: Option<TruncationView<'a>>,
    /// Count unit in the warning.
    pub(super) noun: &'static str,
    /// Rows shown before expansion.
    pub(super) preview_lines: usize,
    /// Optional warning after count and bytes.
    pub(super) trailing_warning: Option<&'a str>,
}
/// Select a listing body without a separate line cap and append its notices.
pub(super) fn select_listing(
    body: &str,
    mut notices: Vec<String>,
    trailing_notice: Option<&str>,
) -> (String, Option<TruncationResult>) {
    let cut = truncate_head(
        body,
        TruncationOptions {
            max_lines: Some(usize::try_from(9_007_199_254_740_991_u64).unwrap_or(usize::MAX)),
            max_bytes: None,
        },
    );
    if cut.truncated {
        notices.push(format!(
            "{} limit reached",
            format_size(float(DEFAULT_MAX_BYTES))
        ));
    }
    notices.extend(trailing_notice.map(str::to_owned));
    let (mut output, truncation) = if cut.truncated {
        (cut.content.clone(), Some(cut))
    } else {
        (cut.content, None)
    };
    if !notices.is_empty() {
        output = format!("{output}\n\n[{}]", notices.join(". "));
    }
    (output, truncation)
}
/// Entry-limit, byte-limit or trailing notices, including clipping-only warnings.
fn limit_warning(view: ListingView<'_>, theme: &LiveTheme) -> Rendered<String> {
    let entries = view.count.filter(|limit| *limit != 0.0 && !limit.is_nan());
    let bytes = view.truncation;
    if entries.is_none() && bytes.is_none() && view.trailing_warning.is_none() {
        return Ok(String::new());
    }
    let warnings: Vec<String> = entries
        .map(|limit| format!("{} {} limit", num(limit), view.noun))
        .into_iter()
        .chain(bytes.map(|cut| format!("{} limit", format_size(cut.max_bytes()))))
        .chain(view.trailing_warning.map(str::to_owned))
        .collect();
    let notice = format!("[Truncated: {}]", warnings.join(", "));
    let resolved = themed(theme.get())?;
    Ok(format!(
        "\n{}",
        paint(&resolved, &ThemeColor::Warning, &notice)?
    ))
}

/// The result text: a preview of the listing, an overflow hint and the limit warning.
pub(super) fn render_listing<D>(
    result: &ToolRenderResultLike<'_, D>,
    options: ToolRenderResultOptions,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
    view: ListingView<'_>,
) -> Rendered<String> {
    let output = get_text_output(
        Some(result),
        context.show_images,
        &context.environment.terminal_image,
    )?;
    let output = output.trim_matches(is_whitespace_scalar);
    let mut text = String::new();
    if !output.is_empty() {
        let theme = &*themed(theme.get())?;
        let lines: Vec<&str> = output.split('\n').collect();
        let shown = if options.expanded {
            lines.len()
        } else {
            view.preview_lines.min(lines.len())
        };
        let rows = lines[..shown]
            .iter()
            .map(|line| paint(theme, &ThemeColor::ToolOutput, line))
            .collect::<Rendered<Vec<_>>>()?;
        text = ["\n", &rows.join("\n")].concat();
        if lines.len() > shown {
            let counts = format!("{} more lines", lines.len() - shown);
            text += &expand_hint(&counts, theme, &context.environment.live_theme)?;
        }
    }
    Ok(text + &limit_warning(view, theme)?)
}
