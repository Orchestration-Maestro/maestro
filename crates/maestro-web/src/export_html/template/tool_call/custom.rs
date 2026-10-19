//! Trusted supplied renderer HTML and literal structured fallback.
use super::{format_expandable_output, html, result};
use maestro_theme::SyntaxHighlighter;
use serde_json::Value;
use std::{fmt::Write as _, io};

/// Select supplied bodies without executing a renderer or trusting session-authored text.
pub(super) fn render_custom(
    call: &Value,
    args: &Value,
    result: Option<&Value>,
    rendered: Option<&Value>,
    highlighter: &dyn SyntaxHighlighter,
) -> io::Result<String> {
    let field = |key: &str| {
        rendered
            .and_then(|r| r[key].as_str())
            .filter(|s| !s.is_empty())
    };
    let (call_html, collapsed, expanded) = (
        field("callHtml"),
        field("resultHtmlCollapsed"),
        field("resultHtmlExpanded"),
    );
    let supplied = call_html.is_some() || collapsed.is_some() || expanded.is_some();
    let mut body = call_html.map_or_else(
        || {
            format!(
                "<div class=\"tool-header\"><span class=\"tool-name\">{}</span></div>",
                html(call["name"].as_str().unwrap_or(""))
            )
        },
        |s| format!("<div class=\"tool-header ansi-rendered\">{s}</div>"),
    );
    if !supplied {
        let args = maestro_app::pretty_json(args).map_err(io::Error::other)?;
        let _ = write!(
            body,
            "<div class=\"tool-output\"><pre>{}</pre></div>",
            html(&args)
        );
    }
    match (collapsed, expanded) {
        (Some(collapsed), Some(expanded)) if collapsed != expanded => {
            let _ = write!(
                body,
                "<div class=\"tool-output expandable ansi-rendered\"><div class=\"output-preview\">{collapsed}</div><div class=\"output-full\">{expanded}</div></div>"
            );
        }
        (_, Some(expanded)) => {
            let _ = write!(
                body,
                "<div class=\"tool-output ansi-rendered\">{expanded}</div>"
            );
        }
        _ => {
            let output = result::get_result_text(result)?;
            if !output.is_empty() {
                body.push_str(&format_expandable_output(&output, 10, None, highlighter));
            }
        }
    }
    Ok(body)
}
