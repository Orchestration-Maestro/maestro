//! Shared owned document to transcript HTML, with contextual math.
use super::admission::{
    Formula, MathBlocks, MathInline, block_formula, matching, scan, starts_display,
};
use maestro_app::markdown::{self, Alignment, InlineAtom, InlineContext, InlineScope, Kind, Node};
use maestro_app::{
    browser::script_scheme,
    is_whitespace_scalar,
    markup::{EscapeMode, escape_into},
    normalize_to_lf,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use std::{fmt::Write as _, ops::Range};

pub(super) mod export;
mod table;
mod walk;

/// Characters the browser's URL encoder leaves untouched, plus the percent sign.
const URL_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b';')
    .remove(b',')
    .remove(b'/')
    .remove(b'?')
    .remove(b':')
    .remove(b'@')
    .remove(b'&')
    .remove(b'=')
    .remove(b'+')
    .remove(b'$')
    .remove(b'#')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'%');

/// Formula errors resolve the selected pack's error role when painted, so a pack or mode
/// change needs no new markup.
const ERROR_COLOR: &str = "var(--maestro-color-error)";

/// One code block in document order, replaced by a native code owner when mounted.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Code {
    /// Original code, never entity-decoded a second time.
    pub(super) code: String,
    /// First information word, or `text`.
    pub(super) language: String,
}
/// Transcript HTML with its code block slots.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Prepared {
    /// Document HTML with one empty slot per code block.
    pub(super) html: String,
    /// Code blocks in slot order.
    pub(super) code: Vec<Code>,
    /// Attribute name absent from authored input, so only generated slots own it.
    pub(super) slot_attribute: String,
}

/// Where an inline run sits, deciding whether display formulas open blocks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// A paragraph in the document, a quote or a loose item.
    Paragraph,
    /// A tight item, which opens block math only when it starts with it.
    Item,
    /// A table cell whose delimiter padding is not display content.
    Cell,
    /// A heading, which is inline only.
    Inline,
}

/// Escape text for HTML content and attribute values.
fn escape(text: &str, out: &mut String) {
    escape_into(text, out, EscapeMode::HtmlDecimal);
}

/// Escape authored chunks without materializing a contiguous node value.
fn escape_authored(node: &Node, out: &mut String) {
    for chunk in node.authored.chunks() {
        escape(chunk, out);
    }
}

/// Prepare `content` as transcript HTML.
pub(super) fn prepare(content: &str, escape_html: bool) -> Prepared {
    let source = normalize_to_lf(content);
    let lower = source.to_ascii_lowercase();
    let mut slot_attribute = "data-maestro-listing".to_owned();
    while lower.contains(&slot_attribute) {
        slot_attribute.push('x');
    }
    let document = match markdown::parse_with_tokenizer(
        source,
        markdown::MarkedOptions::Browser { escape_html },
        Some(&mut MathBlocks),
    ) {
        Ok(document) => document,
        Err(error) => {
            return Prepared {
                html: format!("<p>{error}</p>"),
                slot_attribute,
                ..Prepared::default()
            };
        }
    };

    let inline = InlineContext::new(&document);
    let math = super::math::context();
    let mut writer = Writer {
        policy: Policy::Transcript {
            math: &math,
            inline: &inline,
            escape_html,
        },
        prepared: Prepared {
            slot_attribute,
            ..Prepared::default()
        },
    };
    let mut html = String::new();
    writer.blocks(&document.nodes, None, &mut html);
    writer.prepared.html = html;
    writer.prepared
}

/// Local atom operands; generated node source retains selected formula spelling.
type Atoms<'a, 'b> = Option<&'b [InlineAtom<'a>]>;

/// Rendering behavior selected by the containing surface.
enum Policy<'a, 'd> {
    /// Retained chat controls and formulas.
    Transcript {
        /// Formula renderer for retained chat.
        math: &'a katex::KatexContext,
        /// Original container and label scopes, used only by chat.
        inline: &'a InlineContext<'d>,
        /// Whether authored HTML is shown as text.
        escape_html: bool,
    },
    /// Standalone selectable session markup.
    Export(&'a dyn maestro_theme::SyntaxHighlighter),
}

/// Document walker with the math renderer and generated-slot table.
struct Writer<'a, 'd> {
    /// Surface-specific admission and presentation.
    policy: Policy<'a, 'd>,
    /// Output under construction.
    prepared: Prepared,
}
impl Writer<'_, '_> {
    /// Reserve a listing slot for a code block.
    fn code(&mut self, code: &str, info: Option<&str>, out: &mut String) {
        if let Policy::Export(highlighter) = self.policy {
            export::code(code, info, highlighter, out);
            return;
        }
        let language = info
            .and_then(|info| info.split(is_whitespace_scalar).next())
            .filter(|word| !word.is_empty())
            .unwrap_or("text");
        let _ = writeln!(
            out,
            "<div class=\"maestro-code-slot\" {}=\"{}\"></div>",
            self.prepared.slot_attribute,
            self.prepared.code.len()
        );
        let mut code = code.to_owned();
        if !code.is_empty() && !code.ends_with('\n') {
            code.push('\n');
        }
        self.prepared.code.push(Code {
            code,
            language: language.to_owned(),
        });
    }
    /// Write one table row.
    fn row(
        &mut self,
        cells: &[markdown::Cell],
        (alignments, tag): (&[Alignment], &str),
        atoms: Atoms<'_, '_>,
        out: &mut String,
    ) {
        out.push_str("<tr>\n");
        for (cell, alignment) in cells.iter().zip(alignments) {
            let align = match alignment {
                Alignment::Left => " align=\"left\"",
                Alignment::Center => " align=\"center\"",
                Alignment::Right => " align=\"right\"",
                Alignment::None => "",
            };
            let _ = write!(out, "<{tag}{align}>");
            self.flow(&cell.children, (&cell.range, Flow::Cell), atoms, out);
            let _ = writeln!(out, "</{tag}>");
        }
        out.push_str("</tr>\n");
    }

    /// Write a contextual run using the document's captured original scope.
    fn flow(
        &mut self,
        children: &[Node],
        (range, flow): (&Range<usize>, Flow),
        atoms: Atoms<'_, '_>,
        out: &mut String,
    ) {
        if matches!(self.policy, Policy::Export(_)) {
            let mut body = String::new();
            self.inline(children, atoms, &mut body);
            wrapped(&body, flow == Flow::Paragraph, out);
            return;
        }
        if atoms.is_some() {
            let mut body = String::new();
            self.inline(children, atoms, &mut body);
            wrapped(&body, flow == Flow::Paragraph, out);
            return;
        }
        let Policy::Transcript { inline, .. } = self.policy else {
            return;
        };
        let Some(scope) = inline.scope(range, false) else {
            self.inline(children, None, out);
            return;
        };
        let source = scope.text();
        let content = scope.content_range();
        let end = content.end;
        let block = flow == Flow::Paragraph
            || (flow == Flow::Item && starts_display(&source[content.clone()]));
        let displays = if block {
            scan(scope, content.clone())
        } else {
            Vec::new()
        };
        let wrap = flow == Flow::Paragraph;
        if displays.is_empty() {
            self.piece(scope, content, wrap, out);
            return;
        }
        let mut begin = content.start;
        for formula in &displays {
            let trimmed = source[begin..formula.range.start]
                .trim_end_matches('\n')
                .len();
            let range = begin..begin + trimmed;
            self.piece(scope, range, wrap, out);
            out.push_str(&self.math(formula));
            begin = formula.range.end;
        }
        self.piece(scope, begin..end, wrap, out);
    }
    /// Interpret a local slice with admitted formulas and retained native operands as atoms.
    fn piece(
        &mut self,
        scope: &InlineScope<'_>,
        range: Range<usize>,
        wrap: bool,
        out: &mut String,
    ) {
        if range.is_empty() {
            return;
        }
        let mut extension = MathInline;
        let run = scope.interpret_with_tokenizer(range, &mut extension);
        let inline = run
            .nodes
            .iter()
            .all(|node| matches!(node.kind, Kind::Paragraph(_)));
        let mut body = String::new();
        for node in &run.nodes {
            if let Kind::Paragraph(c) = &node.kind {
                self.inline(c, Some(&run.atoms), &mut body);
            } else {
                self.blocks(std::slice::from_ref(node), Some(&run.atoms), &mut body);
            }
        }
        wrapped(&body, wrap && inline, out);
    }

    /// Write inline nodes.
    fn inline(&mut self, nodes: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        for node in nodes {
            self.inline_node(node, atoms, out);
        }
    }
    /// Write one inline node under the selected surface policy.
    fn inline_node(&mut self, node: &Node, atoms: Atoms<'_, '_>, out: &mut String) {
        match &node.kind {
            Kind::Text(text) => {
                if matches!(self.policy, Policy::Export(_)) && text.authored == "\n" {
                    out.push_str("<br>");
                } else {
                    escape(&text.decoded, out);
                }
            }
            Kind::Code(code) => {
                out.push_str("<code>");
                escape(code, out);
                out.push_str("</code>");
            }
            Kind::Break => out.push_str("<br>"),
            Kind::Strong(c) => self.tagged("strong", c, atoms, out),
            Kind::Strike(c) => self.tagged("del", c, atoms, out),
            Kind::Emphasis(c) => self.tagged("em", c, atoms, out),
            Kind::Link(..) => self.link(node, atoms, out),
            Kind::Image(alt, meta) => match self.policy {
                Policy::Transcript { .. } => image(meta, out),
                Policy::Export(_) => export::image(alt, meta, out),
            },
            Kind::TaskListMarker(checked) => task(*checked, out),
            Kind::Html(html) => match self.policy {
                Policy::Transcript { .. } => out.push_str(html),
                Policy::Export(_) => escape(html, out),
            },
            Kind::Atom(index) => self.atom((node, *index), atoms, out),
            _ => {}
        }
    }
    /// Write an anchor that opens separately with the safe relation.
    fn link(&mut self, node: &Node, atoms: Atoms<'_, '_>, out: &mut String) {
        let Kind::Link(children, _, href, meta) = &node.kind else {
            return;
        };
        if matches!(self.policy, Policy::Export(_)) {
            if let Some(href) = export::sanitized(href.trim_matches(is_whitespace_scalar)) {
                out.push_str("<a href=\"");
                escape(&href, out);
                out.push('"');
                title(&meta.title, out);
                out.push('>');
                self.inline(children, None, out);
                out.push_str("</a>");
            } else {
                self.inline(children, None, out);
            }
            return;
        }
        let Policy::Transcript {
            inline,
            escape_html,
            ..
        } = self.policy
        else {
            return;
        };
        // A refused target keeps the label; with escaping on, an angle autolink keeps its source.
        let href = utf8_percent_encode(href, URL_SAFE).to_string();
        let refused = maestro_app::browser::executable(&href);
        if refused && escape_html && meta.link_type == markdown::LinkType::Autolink {
            escape_authored(node, out);
            return;
        }
        if !refused {
            out.push_str("<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"");
            escape(&href, out);
            out.push('"');
            title(&meta.title, out);
            out.push('>');
        }
        if let Some(scope) = inline.scope(&node.range, true) {
            let content = scope.content_range();
            self.piece(scope, content, false, out);
        } else {
            self.inline(children, atoms, out);
        }
        out.push_str(if refused { "" } else { "</a>" });
    }
    /// Write formatted children inside `tag`.
    fn tagged(&mut self, tag: &str, children: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        let _ = write!(out, "<{tag}>");
        self.inline(children, atoms, out);
        let _ = write!(out, "</{tag}>");
    }
    /// Emit a typed local atom, preserving resolved operands without lookup by display text.
    fn atom(&mut self, (node, index): (&Node, usize), atoms: Atoms<'_, '_>, out: &mut String) {
        let Some(atoms) = atoms else {
            return;
        };
        match atoms.get(index) {
            Some(InlineAtom::Opaque(_)) => {
                if let Some(formula) = matching(&node.authored.to_owned_string(), 0, false) {
                    out.push_str(&self.math(&formula));
                }
            }
            Some(InlineAtom::Resolved(node)) => {
                self.inline(std::slice::from_ref(node), None, out);
            }
            None => {}
        }
    }

    /// Render one formula; a reached renderer failure keeps the delimiter-spelled source.
    fn math(&self, formula: &Formula<'_>) -> String {
        let display = formula.delimiter.display();
        let settings = katex::Settings::builder()
            .display_mode(display)
            .output(katex::OutputFormat::Html)
            .throw_on_error(false)
            .error_color(ERROR_COLOR.to_owned())
            .build();
        let Policy::Transcript { math, .. } = &self.policy else {
            return String::new();
        };
        let rendered = super::math::render_to_string(math, formula.tex, &settings);
        match (rendered, display) {
            (Ok(html), true) => format!("<div class=\"maestro-math-block\">{html}</div>"),
            (Ok(html), false) => html,
            (Err(_), _) => {
                let (open, close) = formula.delimiter.pair();
                let mut text = String::new();
                escape(&format!("{open}{}{close}", formula.tex), &mut text);
                let tag = if display { "div" } else { "span" };
                format!("<{tag} class=\"maestro-math-fallback\">{text}</{tag}>")
            }
        }
    }
}
/// Write a disabled task checkbox and the space before its text.
fn task(checked: bool, out: &mut String) {
    out.push_str(if checked {
        "<input checked=\"\" disabled=\"\" type=\"checkbox\"> "
    } else {
        "<input disabled=\"\" type=\"checkbox\"> "
    });
}
/// Write an image with its plain alternative text, or only that text for a refused target.
fn image(meta: &markdown::ImageMetadata, out: &mut String) {
    let destination = utf8_percent_encode(&meta.destination, URL_SAFE).to_string();
    if url::Url::parse(&destination).is_ok_and(|url| script_scheme(url.scheme())) {
        return plain(&meta.children, out);
    }
    out.push_str("<img src=\"");
    escape(&destination, out);
    out.push_str("\" alt=\"");
    plain(&meta.children, out);
    out.push('"');
    title(&meta.title, out);
    out.push('>');
}
/// Wrap a paragraph body.
fn wrapped(body: &str, paragraph: bool, out: &mut String) {
    if paragraph {
        let _ = writeln!(out, "<p>{body}</p>");
    } else {
        out.push_str(body);
    }
}
/// Append an escaped tooltip attribute when one exists.
fn title(text: &str, out: &mut String) {
    if !text.is_empty() {
        out.push_str(" title=\"");
        escape(text, out);
        out.push('"');
    }
}
/// Escaped text content of formatted children.
fn plain(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match &node.kind {
            Kind::Text(text) => escape(&text.decoded, out),
            Kind::Code(code) => escape(code, out),
            Kind::Strong(c) | Kind::Strike(c) | Kind::Emphasis(c) | Kind::Link(c, ..) => {
                plain(c, out);
            }
            _ => {}
        }
    }
}
