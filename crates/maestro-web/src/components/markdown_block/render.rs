//! Shared owned document to transcript HTML, with contextual math.
use super::admission::{Formula, escape_tags, scan, starts_display};
use maestro_app::{
    is_whitespace_scalar,
    markdown::{self, Alignment, Kind, Node, Table},
    markup::{EscapeMode, escape_into},
    normalize_to_lf,
};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use std::{fmt::Write as _, ops::Range};

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
}

/// Where an inline run sits, deciding whether display formulas open blocks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    /// A paragraph in the document, a quote or a loose item.
    Paragraph,
    /// A tight item, which opens block math only when it starts with it.
    Item,
    /// A heading or table cell, which is inline only.
    Inline,
}

/// Escape text for HTML content and attribute values.
fn escape(text: &str, out: &mut String) {
    escape_into(text, out, EscapeMode::HtmlDecimal);
}

/// Prepare `content` as transcript HTML.
pub(super) fn prepare(content: &str, escape_html: bool) -> Prepared {
    let source = normalize_to_lf(content);
    let source = if escape_html {
        escape_tags(source)
    } else {
        source
    };
    let mut document = markdown::parse(source);
    markdown::autolinks(&mut document);
    let mut writer = Writer {
        context: katex::KatexContext::default(),
        slots: Vec::new(),
        prepared: Prepared::default(),
    };
    let mut html = String::new();
    writer.blocks(&document.nodes, &mut html);
    writer.prepared.html = html;
    writer.prepared
}

/// Document walker with the math renderer and generated-slot table.
struct Writer {
    /// Reused font and macro tables.
    context: katex::KatexContext,
    /// Generated inline slots of the document currently being written.
    slots: Vec<(Range<usize>, Formula)>,
    /// Output under construction.
    prepared: Prepared,
}
impl Writer {
    /// Write block nodes in order.
    fn blocks(&mut self, nodes: &[Node], out: &mut String) {
        for node in nodes {
            self.block(node, out);
        }
    }
    /// Write one block node.
    fn block(&mut self, node: &Node, out: &mut String) {
        match &node.kind {
            Kind::Paragraph(c) => self.flow(c, &node.authored, Flow::Paragraph, out),
            Kind::Inline(c) => self.flow(c, &node.authored, Flow::Item, out),
            Kind::Heading(level, c, authored) => {
                let _ = write!(out, "<h{level}>");
                self.flow(c, authored, Flow::Inline, out);
                let _ = writeln!(out, "</h{level}>");
            }
            Kind::List(start, items) => {
                let (tag, attr) = match start {
                    None => ("ul", String::new()),
                    Some(1) => ("ol", String::new()),
                    Some(n) => ("ol", format!(" start=\"{n}\"")),
                };
                let _ = writeln!(out, "<{tag}{attr}>");
                self.blocks(items, out);
                let _ = writeln!(out, "</{tag}>");
            }
            Kind::Item(c) => {
                out.push_str("<li>");
                self.blocks(c, out);
                out.push_str("</li>\n");
            }
            Kind::Quote(c, _) => {
                out.push_str("<blockquote>\n");
                self.blocks(c, out);
                out.push_str("</blockquote>\n");
            }
            Kind::HtmlBlock(html) => out.push_str(html),
            Kind::Rule => out.push_str("<hr>\n"),
            Kind::CodeBlock(code, info) => self.code(code, info.as_deref(), out),
            Kind::Table(table) => self.table(table, out),
            Kind::Gap => {}
            _ => self.inline(std::slice::from_ref(node), out),
        }
    }
    /// Reserve a listing slot for a code block.
    fn code(&mut self, code: &str, info: Option<&str>, out: &mut String) {
        let language = info
            .and_then(|info| info.split(is_whitespace_scalar).next())
            .filter(|word| !word.is_empty())
            .unwrap_or("text");
        let _ = writeln!(
            out,
            "<div class=\"maestro-code-slot\" data-code=\"{}\"></div>",
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
    /// Write a table inside its own horizontal scroller.
    fn table(&mut self, table: &Table, out: &mut String) {
        out.push_str("<div class=\"maestro-table-scroll\"><table>\n<thead>\n");
        self.row(&table.header, &table.alignments, "th", out);
        out.push_str("</thead>\n<tbody>");
        for row in &table.rows {
            self.row(row, &table.alignments, "td", out);
        }
        out.push_str("</tbody></table>\n</div>");
    }
    /// Write one table row.
    fn row(
        &mut self,
        cells: &[markdown::Cell],
        alignments: &[Alignment],
        tag: &str,
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
            self.flow(&cell.children, &cell.authored, Flow::Inline, out);
            let _ = writeln!(out, "</{tag}>");
        }
        out.push_str("</tr>\n");
    }

    /// Write one inline run, admitting display formulas as the context allows.
    fn flow(&mut self, children: &[Node], authored: &str, flow: Flow, out: &mut String) {
        let source = if flow == Flow::Paragraph {
            authored.trim_end_matches('\n')
        } else {
            authored
        };
        let block = flow == Flow::Paragraph || (flow == Flow::Item && starts_display(source));
        let displays = if block {
            scan(source, true)
        } else {
            Vec::new()
        };
        let wrap = flow == Flow::Paragraph;
        if displays.is_empty() {
            let formulas = scan(source, false);
            let mut body = String::new();
            if formulas.is_empty() {
                self.inline(children, &mut body);
            } else {
                self.reparsed(source, &formulas, false, &mut body);
            }
            wrapped(&body, wrap, out);
            return;
        }
        let mut end = 0;
        for formula in &displays {
            self.piece(
                source[end..formula.range.start].trim_end_matches('\n'),
                wrap,
                out,
            );
            out.push_str(&self.math(formula));
            end = formula.range.end;
        }
        self.piece(&source[end..], wrap, out);
    }
    /// Write a text piece between display formulas.
    fn piece(&mut self, text: &str, wrap: bool, out: &mut String) {
        if text.is_empty() {
            return;
        }
        let formulas = scan(text, false);
        let mut body = String::new();
        self.reparsed(text, &formulas, true, &mut body);
        wrapped(&body, wrap, out);
    }

    /// Parse `source` again with each formula replaced by an owned generated slot.
    fn reparsed(&mut self, source: &str, formulas: &[Formula], edges: bool, out: &mut String) {
        let (mut generated, mut end) = (String::new(), 0);
        let mut slots = Vec::new();
        for formula in formulas {
            generated.push_str(&source[end..formula.range.start]);
            let start = generated.len();
            generated.push_str("<m></m>");
            slots.push((start..generated.len(), formula.clone()));
            end = formula.range.end;
        }
        generated.push_str(&source[end..]);
        let mut document = markdown::parse(generated);
        markdown::autolinks(&mut document);
        let outer = std::mem::replace(&mut self.slots, slots);
        let edge = |c: char| c == ' ' || c == '\t';
        let leading = &source[..source.len() - source.trim_start_matches(edge).len()];
        let trailing = &source[source.trim_end_matches([' ', '\t', '\n']).len()..];
        if edges {
            escape(leading, out);
        }
        for node in &document.nodes {
            match &node.kind {
                Kind::Paragraph(children) => self.inline(children, out),
                _ => self.block(node, out),
            }
        }
        if edges {
            escape(trailing, out);
        }
        self.slots = outer;
    }

    /// Write inline nodes.
    fn inline(&mut self, nodes: &[Node], out: &mut String) {
        for node in nodes {
            match &node.kind {
                Kind::Text(text) => escape(&text.decoded, out),
                Kind::Code(code) => {
                    out.push_str("<code>");
                    escape(code, out);
                    out.push_str("</code>");
                }
                Kind::Break => out.push_str("<br>"),
                Kind::Strong(c) => self.tagged("strong", c, out),
                Kind::Strike(c) => self.tagged("del", c, out),
                Kind::Emphasis(c) => self.tagged("em", c, out),
                Kind::Link(c, _, href, meta) => self.link(c, (href, &meta.title), out),
                Kind::Image(_, meta) => image(meta, out),
                Kind::TaskListMarker(checked) => task(*checked, out),
                Kind::Html(html) => self.html(node, html, out),
                _ => {}
            }
        }
    }
    /// Write an anchor that opens separately with the safe relation.
    fn link(&mut self, children: &[Node], (href, tooltip): (&str, &str), out: &mut String) {
        out.push_str("<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"");
        escape(&utf8_percent_encode(href, URL_SAFE).to_string(), out);
        out.push('"');
        title(tooltip, out);
        out.push('>');
        self.inline(children, out);
        out.push_str("</a>");
    }
    /// Write formatted children inside `tag`.
    fn tagged(&mut self, tag: &str, children: &[Node], out: &mut String) {
        let _ = write!(out, "<{tag}>");
        self.inline(children, out);
        let _ = write!(out, "</{tag}>");
    }
    /// Emit a generated slot's formula once, or authored inline markup verbatim.
    fn html(&self, node: &Node, html: &str, out: &mut String) {
        let slot = self.slots.iter().find(|(range, _)| match html {
            "<m>" => range.start == node.range.start,
            "</m>" => range.end == node.range.end,
            _ => false,
        });
        match slot {
            Some((range, formula)) if range.start == node.range.start => {
                out.push_str(&self.math(formula));
            }
            Some(_) => {}
            None => out.push_str(html),
        }
    }

    /// Render one formula; a reached renderer failure keeps the delimiter-spelled source.
    fn math(&self, formula: &Formula) -> String {
        let display = formula.delimiter.display();
        let settings = katex::Settings::builder()
            .display_mode(display)
            .output(katex::OutputFormat::Html)
            .throw_on_error(false)
            .error_color(ERROR_COLOR.to_owned())
            .build();
        let rendered = katex::render_to_string(&self.context, &formula.tex, &settings);
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
/// Write an image with its plain alternative text.
fn image(meta: &markdown::ImageMetadata, out: &mut String) {
    out.push_str("<img src=\"");
    escape(
        &utf8_percent_encode(&meta.destination, URL_SAFE).to_string(),
        out,
    );
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
