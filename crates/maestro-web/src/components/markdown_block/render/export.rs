//! Viewer-specific admission through the shared document walker.
use super::{Policy, Prepared, Writer, escape, title};
use maestro_app::{is_whitespace_scalar, markdown, normalize_to_lf};
use maestro_theme::{HighlightProfile, SyntaxHighlighter};

/// Prepare standalone viewer markup without retained chat controls.
pub(crate) fn prepare_export(text: &str, highlighter: &dyn SyntaxHighlighter) -> String {
    let mut document = markdown::parse(super::escape_tags(normalize_to_lf(text), true));
    markdown::autolinks(&mut document);
    let inline = markdown::InlineContext::new(&document);
    let mut writer = Writer {
        policy: Policy::Export(highlighter),
        inline: &inline,
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

impl super::Writer<'_, '_> {
    /// Render one original inline scope with strict viewer admission.
    pub(super) fn export_flow(
        &mut self,
        children: &[markdown::Node],
        (range, flow): (&std::ops::Range<usize>, super::Flow),
        atoms: super::Atoms<'_, '_>,
        out: &mut String,
    ) {
        if let Some(scope) = self.inline.scope(range, false) {
            let mut body = String::new();
            let source = scope.text();
            let range = if flow == super::Flow::Cell {
                let start = source.len() - source.trim_start_matches(is_whitespace_scalar).len();
                start..start + source.trim_matches(is_whitespace_scalar).len()
            } else {
                0..source.trim_end_matches('\n').len()
            };
            self.export_range(scope, range, &mut body);
            super::wrapped(&body, flow == super::Flow::Paragraph, out);
        } else {
            self.inline(children, atoms, out);
        }
    }

    /// Interpret strict deletion without allowing an operand's interior to close it.
    pub(super) fn export_range(
        &mut self,
        scope: &markdown::InlineScope<'_>,
        range: std::ops::Range<usize>,
        out: &mut String,
    ) {
        let spans = deletions(scope, range.clone());
        let run = scope.inline_tokens(range, &spans);
        for node in &run.nodes {
            match &node.kind {
                markdown::Kind::Paragraph(c) => self.inline(
                    c,
                    Some(super::Operands::Export(&run.atoms, &spans, scope)),
                    out,
                ),
                _ => self.inline(
                    std::slice::from_ref(node),
                    Some(super::Operands::Export(&run.atoms, &spans, scope)),
                    out,
                ),
            }
        }
    }

    /// Keep the authored label's resolved scope when applying export inline rules.
    pub(super) fn export_label(
        &mut self,
        node: &markdown::Node,
        children: &[markdown::Node],
        out: &mut String,
    ) {
        if let Some(scope) = self.inline.scope(&node.range, true) {
            self.export_range(scope, 0..scope.text().len(), out);
        } else {
            self.inline(children, None, out);
        }
    }
}

/// Restore native deletion markers that the stricter viewer did not admit.
pub(super) fn literal_strike(
    node: &markdown::Node,
    out: &mut String,
    children: impl FnOnce(&mut String),
) {
    let count = node.authored.bytes().take_while(|b| *b == b'~').count();
    let marker = if count == 1 { "~" } else { "~~" };
    out.push_str(marker);
    children(out);
    out.push_str(marker);
}

/// Strict deletion extents, selected with monotonically advancing operand positions.
fn deletions(
    scope: &markdown::InlineScope<'_>,
    range: std::ops::Range<usize>,
) -> Vec<std::ops::Range<usize>> {
    let source = scope.text();
    let mut spans = Vec::new();
    let mut index = range.start;
    let mut operand = scope.operands.partition_point(|(r, _)| r.end <= index);
    while index < range.end {
        if let Some((span, _)) = scope
            .operands
            .get(operand)
            .filter(|(r, _)| r.start <= index)
        {
            index = span.end;
            operand += 1;
            continue;
        }
        let rest = &source[index..range.end];
        if rest.starts_with('\\') {
            index += escaped_len(rest);
        } else if rest.starts_with("~~") {
            if let Some(end) = (!rest.starts_with("~~~"))
                .then(|| deletion_end(scope, index + 2..range.end, operand))
                .flatten()
            {
                spans.push(index..end);
                index = end;
                operand = scope.operands.partition_point(|(r, _)| r.end <= index);
            } else {
                index += rest.bytes().take_while(|b| *b == b'~').count();
            }
        } else {
            index += rest.chars().next().map_or(1, char::len_utf8);
        }
    }
    spans
}

/// Length of one escaped scalar pair, or a trailing backslash.
fn escaped_len(source: &str) -> usize {
    1 + source[1..].chars().next().map_or(0, char::len_utf8)
}

/// Find a closer outside resolved operands with strict first and last token edges.
fn deletion_end(
    scope: &markdown::InlineScope<'_>,
    range: std::ops::Range<usize>,
    mut operand: usize,
) -> Option<usize> {
    let source = scope.text();
    let first = source[range.clone()].chars().next()?;
    if first == '~' || is_whitespace_scalar(first) {
        return None;
    }
    let mut index = range.start;
    let mut edge = false;
    while index < range.end {
        if let Some((span, _)) = scope
            .operands
            .get(operand)
            .filter(|(r, _)| r.start <= index)
        {
            index = span.end;
            operand += 1;
            edge = true;
            continue;
        }
        let rest = &source[index..range.end];
        if rest.starts_with("~~") && !rest.starts_with("~~~") && edge {
            return Some(index + 2);
        }
        let ch = rest.chars().next()?;
        if ch == '\\' {
            if rest.starts_with("\\\n") {
                return None;
            }
            index += escaped_len(rest);
            edge = true;
        } else {
            edge = ch != '~' && !is_whitespace_scalar(ch);
            index += ch.len_utf8();
        }
    }
    None
}
