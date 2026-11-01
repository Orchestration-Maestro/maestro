//! Viewer-specific admission through the shared document walker.
use super::{Policy, Prepared, Writer, escape, title};
use maestro_app::{is_whitespace_scalar, markdown, normalize_to_lf};
use maestro_theme::{HighlightProfile, SyntaxHighlighter};

/// Prepare standalone viewer markup without retained chat controls.
pub(crate) fn prepare_export(text: &str, highlighter: &dyn SyntaxHighlighter) -> String {
    let document =
        markdown::parse_with_options(normalize_to_lf(text), markdown::MarkedOptions::Export);

    let mut writer = Writer {
        policy: Policy::Export(highlighter),
        prepared: Prepared::default(),
    };
    let mut html = String::new();
    writer.blocks(&document.nodes, None, &mut html);
    html
}

/// Schemes an exported link or image target may name; a target without a scheme is relative.
const ALLOWED_SCHEMES: [&str; 5] = ["http", "https", "mailto", "tel", "ftp"];

/// The target with ASCII control characters removed, as a browser reads it, or `None` when it
/// names a scheme outside the allowlist.
pub(super) fn sanitized(href: &str) -> Option<String> {
    let href: String = href.chars().filter(|c| !c.is_ascii_control()).collect();
    let scheme = href.split_once(':').map(|(name, _)| name).filter(|name| {
        name.starts_with(|c: char| c.is_ascii_alphabetic())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    scheme
        .is_none_or(|name| ALLOWED_SCHEMES.iter().any(|a| a.eq_ignore_ascii_case(name)))
        .then_some(href)
}

/// Emit literal image alternatives rather than formatted label children.
pub(super) fn image(alt: &str, meta: &markdown::ImageMetadata, out: &mut String) {
    let Some(href) = sanitized(meta.destination.trim_matches(is_whitespace_scalar)) else {
        escape(alt, out);
        return;
    };
    out.push_str("<img src=\"");
    escape(&href, out);
    out.push_str("\" alt=\"");
    escape(alt, out);
    out.push('"');
    title(&meta.title, out);
    out.push('>');
}

/// Highlight the complete information label using the viewer policy.
pub(super) fn code(
    code: &str,
    info: Option<&str>,
    highlighter: &dyn SyntaxHighlighter,
    out: &mut String,
) {
    let code = code.strip_suffix('\n').unwrap_or(code);
    let html =
        maestro_theme::highlight_auto(highlighter, code, info, HighlightProfile::ExportViewer)
            .unwrap_or_else(|_| {
                maestro_app::markup::escape(code, maestro_app::markup::EscapeMode::HtmlDecimal)
            });
    out.push_str("<pre><code class=\"hljs\">");
    out.push_str(&html);
    out.push_str("</code></pre>");
}

#[cfg(test)]
mod tests;
