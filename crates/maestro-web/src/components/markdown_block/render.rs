//! Shared owned document to transcript HTML, with contextual math.
use super::admission::{Formula, escape_tags, scan, starts_display};
use maestro_app::{
    is_whitespace_scalar,
    markdown::{self, Alignment, InlineAtom, InlineContext, InlineScope, Kind, Node, Table},
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
    let mut document = if escape_html {
        escape_tags(source)
    } else {
        markdown::parse(source)
    };
    let lower = document.source.to_ascii_lowercase();
    let mut slot_attribute = "data-maestro-listing".to_owned();
    while lower.contains(&slot_attribute) {
        slot_attribute.push('x');
    }
    markdown::autolinks(&mut document);
    let inline = InlineContext::new(&document);
    let mut writer = Writer {
        context: katex::KatexContext::default(),
        inline: &inline,
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

/// Local atom operands paired with their caller-admitted formulas.
type Atoms<'a, 'b> = Option<(&'b [InlineAtom<'a>], &'b [Formula])>;

/// Document walker with the math renderer and generated-slot table.
struct Writer<'a, 'd> {
    /// One resolved context with original container and label scopes.
    inline: &'a InlineContext<'d>,
    /// Reused font and macro tables.
    context: katex::KatexContext,
    /// Output under construction.
    prepared: Prepared,
}
impl Writer<'_, '_> {
    /// Write block nodes in order.
    fn blocks(&mut self, nodes: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        for node in nodes {
            self.block(node, atoms, out);
        }
    }
    /// Write one block node.
    fn block(&mut self, node: &Node, atoms: Atoms<'_, '_>, out: &mut String) {
        match &node.kind {
            Kind::Paragraph(c) => self.flow(c, (&node.range, Flow::Paragraph), atoms, out),
            Kind::Inline(c) => self.flow(c, (&node.range, Flow::Item), atoms, out),
            Kind::Heading(level, c, _) => {
                let _ = write!(out, "<h{level}>");
                self.flow(c, (&node.range, Flow::Inline), atoms, out);
                let _ = writeln!(out, "</h{level}>");
            }
            Kind::List(start, items) => {
                let (tag, attr) = match start {
                    None => ("ul", String::new()),
                    Some(1) => ("ol", String::new()),
                    Some(n) => ("ol", format!(" start=\"{n}\"")),
                };
                let _ = writeln!(out, "<{tag}{attr}>");
                self.blocks(items, atoms, out);
                let _ = writeln!(out, "</{tag}>");
            }
            Kind::Item(c) => {
                out.push_str("<li>");
                self.blocks(c, atoms, out);
                out.push_str("</li>\n");
            }
            Kind::Quote(c, _) => {
                out.push_str("<blockquote>\n");
                self.blocks(c, atoms, out);
                out.push_str("</blockquote>\n");
            }
            Kind::HtmlBlock(html) => out.push_str(html),
            Kind::Rule => out.push_str("<hr>\n"),
            Kind::CodeBlock(code, info) => self.code(code, info.as_deref(), out),
            Kind::Table(table) => self.table(table, atoms, out),
            Kind::Gap => {}
            _ => self.inline(std::slice::from_ref(node), atoms, out),
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
    /// Write a table inside its own horizontal scroller.
    fn table(&mut self, table: &Table, atoms: Atoms<'_, '_>, out: &mut String) {
        out.push_str("<div class=\"maestro-table-scroll\"><table>\n<thead>\n");
        self.row(&table.header, (&table.alignments, "th"), atoms, out);
        out.push_str("</thead>\n<tbody>");
        for row in &table.rows {
            self.row(row, (&table.alignments, "td"), atoms, out);
        }
        out.push_str("</tbody></table>\n</div>");
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
            self.flow(&cell.children, (&cell.range, Flow::Inline), atoms, out);
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
        if atoms.is_some() {
            let mut body = String::new();
            self.inline(children, atoms, &mut body);
            wrapped(&body, flow == Flow::Paragraph, out);
            return;
        }
        let Some(scope) = self.inline.scope(range, false) else {
            self.inline(children, None, out);
            return;
        };
        let source = scope.text();
        let end = if flow == Flow::Paragraph {
            source.trim_end_matches('\n').len()
        } else {
            source.len()
        };
        let block = flow == Flow::Paragraph || (flow == Flow::Item && starts_display(source));
        let displays = if block {
            scan(scope, 0..source.len(), true)
        } else {
            Vec::new()
        };
        let wrap = flow == Flow::Paragraph;
        if displays.is_empty() {
            let formulas = scan(scope, 0..scope.text().len(), false);
            if formulas.is_empty() {
                let mut body = String::new();
                self.inline(children, None, &mut body);
                wrapped(&body, wrap, out);
            } else {
                self.piece(scope, 0..end, (&formulas, wrap, false), out);
            }
            return;
        }
        let mut begin = 0;
        for formula in &displays {
            let trimmed = source[begin..formula.range.start]
                .trim_end_matches('\n')
                .len();
            let range = begin..begin + trimmed;
            let formulas = scan(scope, range.clone(), false);
            self.piece(scope, range, (&formulas, wrap, true), out);
            out.push_str(&self.math(formula));
            begin = formula.range.end;
        }
        let formulas = scan(scope, begin..end, false);
        self.piece(scope, begin..end, (&formulas, wrap, true), out);
    }
    /// Interpret a local slice with admitted formulas and retained native operands as atoms.
    fn piece(
        &mut self,
        scope: &InlineScope<'_>,
        range: Range<usize>,
        (formulas, wrap, edges): (&[Formula], bool, bool),
        out: &mut String,
    ) {
        if range.is_empty() {
            return;
        }
        let first = formulas.partition_point(|formula| formula.range.start < range.start);
        let last = formulas.partition_point(|formula| formula.range.end <= range.end);
        let formulas = &formulas[first.min(last)..last];
        let spans: Vec<_> = formulas
            .iter()
            .map(|formula| formula.range.clone())
            .collect();
        let run = scope.interpret(range.clone(), &spans);
        let source = &scope.text()[range];
        let inline = run
            .nodes
            .iter()
            .all(|node| matches!(node.kind, Kind::Paragraph(_)));
        let edges = edges && inline;
        let edge = |c: char| c == ' ' || c == '\t';
        let mut body = String::new();
        if edges {
            escape(
                &source[..source.len() - source.trim_start_matches(edge).len()],
                &mut body,
            );
        }
        for node in &run.nodes {
            match &node.kind {
                Kind::Paragraph(c) => self.inline(c, Some((&run.atoms, formulas)), &mut body),
                _ => self.block(node, Some((&run.atoms, formulas)), &mut body),
            }
        }
        if edges {
            escape(
                &source[source.trim_end_matches([' ', '\t', '\n']).len()..],
                &mut body,
            );
        }
        wrapped(&body, wrap && inline, out);
    }

    /// Write inline nodes.
    fn inline(&mut self, nodes: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        for node in nodes {
            match &node.kind {
                Kind::Text(text) => escape(&text.decoded, out),
                Kind::Code(code) => {
                    out.push_str("<code>");
                    escape(code, out);
                    out.push_str("</code>");
                }
                Kind::Break => out.push_str("<br>"),
                Kind::Strong(c) => self.tagged("strong", c, atoms, out),
                Kind::Strike(c) => self.tagged("del", c, atoms, out),
                Kind::Emphasis(c) => self.tagged("em", c, atoms, out),
                Kind::Link(..) => self.link(node, out),
                Kind::Image(_, meta) => image(meta, out),
                Kind::TaskListMarker(checked) => task(*checked, out),
                Kind::Html(html) => out.push_str(html),
                Kind::Atom(index) => self.atom(*index, atoms, out),
                _ => {}
            }
        }
    }
    /// Write an anchor that opens separately with the safe relation.
    fn link(&mut self, node: &Node, out: &mut String) {
        let Kind::Link(children, _, href, meta) = &node.kind else {
            return;
        };
        out.push_str("<a target=\"_blank\" rel=\"noopener noreferrer\" href=\"");
        escape(&utf8_percent_encode(href, URL_SAFE).to_string(), out);
        out.push('"');
        title(&meta.title, out);
        out.push('>');
        if let Some(scope) = self.inline.scope(&node.range, true) {
            let formulas = scan(scope, 0..scope.text().len(), false);
            if formulas.is_empty() {
                self.inline(children, None, out);
            } else {
                self.piece(scope, 0..scope.text().len(), (&formulas, false, false), out);
            }
        } else {
            self.inline(children, None, out);
        }
        out.push_str("</a>");
    }
    /// Write formatted children inside `tag`.
    fn tagged(&mut self, tag: &str, children: &[Node], atoms: Atoms<'_, '_>, out: &mut String) {
        let _ = write!(out, "<{tag}>");
        self.inline(children, atoms, out);
        let _ = write!(out, "</{tag}>");
    }
    /// Emit a typed local atom, preserving resolved operands without lookup by display text.
    fn atom(&mut self, index: usize, atoms: Atoms<'_, '_>, out: &mut String) {
        let Some((atoms, formulas)) = atoms else {
            return;
        };
        match atoms.get(index) {
            Some(InlineAtom::Opaque(id)) => {
                if let Some(formula) = formulas.get(*id) {
                    out.push_str(&self.math(formula));
                }
            }
            Some(InlineAtom::Resolved(node)) => {
                self.inline(std::slice::from_ref(node), None, out);
            }
            None => {}
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
