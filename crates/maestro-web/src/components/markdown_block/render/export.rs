//! Viewer-specific admission through the shared document walker.
use super::{Policy, Prepared, Writer, escape, title};
use maestro_app::{is_whitespace_scalar, markdown, normalize_to_lf};
use maestro_theme::{HighlightProfile, SyntaxHighlighter};

/// Prepare standalone viewer markup without retained chat controls.
pub(crate) fn prepare_export(text: &str, highlighter: &dyn SyntaxHighlighter) -> String {
    let mut document =
        markdown::parse_with_options(normalize_to_lf(text), markdown::MarkedOptions::Export);
    markdown::autolinks(&mut document);
    let mut writer = Writer {
        policy: Policy::Export(highlighter),
        prepared: Prepared::default(),
    };
    let mut html = String::new();
    writer.blocks(&document.nodes, None, &mut html);
    html
}

/// Classify the operand as consumed by a native HTML URL attribute.
pub(super) fn blocked(href: &str) -> bool {
    let selected = href.replace('\0', "\u{fffd}");
    url::Url::parse(&selected)
        .is_ok_and(|url| matches!(url.scheme(), "javascript" | "vbscript" | "data"))
}

/// Emit literal image alternatives rather than formatted label children.
pub(super) fn image(alt: &str, meta: &markdown::ImageMetadata, out: &mut String) {
    let href = meta.destination.trim_matches(is_whitespace_scalar);
    if blocked(href) {
        escape(alt, out);
        return;
    }
    out.push_str("<img src=\"");
    escape(href, out);
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
