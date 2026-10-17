//! Source preparation and contextual math admission over the shared owned document.
use maestro_app::{
    is_whitespace_scalar,
    markdown::{self, Document, Kind, LinkType, Node},
};
use std::{ops::Range, sync::LazyLock};

/// Tag shapes that are shown as text, in the order they are applied.
static TAGS: LazyLock<[(regress::Regex, &str); 3]> = LazyLock::new(|| {
    let pattern = |source, replacement| {
        (
            regress::Regex::new(source).unwrap_or_else(|_| unreachable!("constant pattern")),
            replacement,
        )
    };
    [
        pattern(r"<(\w+)([^>]*)>", "&lt;$1$2&gt;"),
        pattern(r"</(\w+)>", "&lt;/$1&gt;"),
        pattern(r"<(?![^\s])", "&lt;"),
    ]
});

/// Visit every native node in source order, including table cells.
pub(super) fn visit<'a>(nodes: &'a [Node], f: &mut impl FnMut(&'a Node)) {
    for node in nodes {
        f(node);
        match &node.kind {
            Kind::Paragraph(c)
            | Kind::Inline(c)
            | Kind::Heading(_, c, _)
            | Kind::List(_, c)
            | Kind::Item(c)
            | Kind::Quote(c, _)
            | Kind::Link(c, ..)
            | Kind::Strong(c)
            | Kind::Strike(c)
            | Kind::Emphasis(c) => visit(c, f),
            Kind::Table(table) => {
                for cell in std::iter::once(&table.header).chain(&table.rows).flatten() {
                    visit(&cell.children, f);
                }
            }
            _ => {}
        }
    }
}

/// Source text of a link node, which starts with `[` only for authored labels.
fn authored_label(document: &Document, node: &Node) -> bool {
    document.source[node.range.clone()].starts_with('[')
}

/// Escape tag-like text outside native code and angle autolinks.
pub(super) fn escape_tags(source: String) -> String {
    let document = markdown::parse(source);
    let mut spans = Vec::new();
    visit(&document.nodes, &mut |node| match &node.kind {
        Kind::Code(_) | Kind::CodeBlock(..) => spans.push(node.range.clone()),
        Kind::Link(_, _, _, meta)
            if matches!(meta.link_type, LinkType::Autolink | LinkType::Email)
                && !authored_label(&document, node) =>
        {
            spans.push(node.range.clone());
        }
        _ => {}
    });
    let mut out = String::with_capacity(document.source.len());
    let mut cursor = 0;
    for span in spans {
        if span.start < cursor {
            continue;
        }
        out.push_str(&shown_as_text(&document.source[cursor..span.start]));
        out.push_str(&document.source[span.clone()]);
        cursor = span.end;
    }
    out.push_str(&shown_as_text(&document.source[cursor..]));
    out
}

/// Turn tag-like text into entity text.
fn shown_as_text(text: &str) -> String {
    TAGS.iter()
        .fold(text.to_owned(), |text, (tag, replacement)| {
            tag.replace_all(&text, replacement)
        })
}

/// Delimiter family of one formula.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Delimiter {
    /// `$…$`.
    Dollar,
    /// `\(…\)`.
    Paren,
    /// `$$…$$`.
    DoubleDollar,
    /// `\[…\]`.
    Bracket,
}
impl Delimiter {
    /// Opening and closing spelling.
    pub(super) const fn pair(self) -> (&'static str, &'static str) {
        match self {
            Self::Dollar => ("$", "$"),
            Self::Paren => ("\\(", "\\)"),
            Self::DoubleDollar => ("$$", "$$"),
            Self::Bracket => ("\\[", "\\]"),
        }
    }
    /// Whether the family renders as a display block.
    pub(super) const fn display(self) -> bool {
        matches!(self, Self::DoubleDollar | Self::Bracket)
    }
}

/// An admitted formula with its trimmed body.
#[derive(Clone, Debug)]
pub(super) struct Formula {
    /// Extent in the scanned source.
    pub(super) range: Range<usize>,
    /// Trimmed body, trimmed after the nonempty check.
    pub(super) tex: String,
    /// Delimiter family.
    pub(super) delimiter: Delimiter,
}

/// Match one formula starting exactly at `index`.
fn matching(source: &str, index: usize, display: bool) -> Option<Formula> {
    let families = if display {
        [Delimiter::DoubleDollar, Delimiter::Bracket]
    } else {
        [Delimiter::Dollar, Delimiter::Paren]
    };
    families.into_iter().find_map(|delimiter| {
        let (open, close) = delimiter.pair();
        let rest = source[index..].strip_prefix(open)?;
        let end = rest.find(close)?;
        let body = &rest[..end];
        let dollar = open.starts_with('$');
        let excluded = dollar && (body.contains('$') || (!display && body.contains('\n')));
        (!body.is_empty() && !excluded).then(|| Formula {
            range: index..index + open.len() + end + close.len(),
            tex: body.trim_matches(is_whitespace_scalar).to_owned(),
            delimiter,
        })
    })
}

/// Source ranges where a formula may neither start nor be recognized.
fn opaque(document: &Document) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    visit(&document.nodes, &mut |node| match &node.kind {
        Kind::Code(_)
        | Kind::CodeBlock(..)
        | Kind::Html(_)
        | Kind::HtmlBlock(_)
        | Kind::Image(..) => {
            ranges.push(node.range.clone());
        }
        Kind::Link(children, ..) => match children.last() {
            Some(last) if authored_label(document, node) => {
                ranges.push(node.range.start..node.range.start + 1);
                ranges.push(last.range.end..node.range.end);
            }
            _ => ranges.push(node.range.clone()),
        },
        _ => {}
    });
    ranges.sort_by_key(|range| range.start);
    ranges
}

/// Formulas of one display class outside opaque ranges, in source order.
pub(super) fn scan(source: &str, display: bool) -> Vec<Formula> {
    if !source.contains(['$', '\\']) {
        return Vec::new();
    }
    let mut document = markdown::parse(source.to_owned());
    markdown::autolinks(&mut document);
    let opaque = opaque(&document);
    let (mut found, mut next, mut slashes) = (Vec::new(), 0, 0_usize);
    let mut index = 0;
    while index < source.len() {
        while opaque.get(next).is_some_and(|range| range.end <= index) {
            next += 1;
        }
        if let Some(range) = opaque.get(next).filter(|r| r.contains(&index)) {
            index = range.end;
            slashes = 0;
            continue;
        }
        let formula = (slashes % 2 == 0)
            .then(|| matching(source, index, display))
            .flatten();
        if let Some(formula) = formula {
            index = formula.range.end;
            slashes = 0;
            found.push(formula);
            continue;
        }
        let ch = source[index..].chars().next().unwrap_or_default();
        slashes = if ch == '\\' { slashes + 1 } else { 0 };
        index += ch.len_utf8().max(1);
    }
    found
}

/// Whether a tight item starts with a display formula and therefore opens block math.
pub(super) fn starts_display(source: &str) -> bool {
    matching(source, 0, true).is_some()
}
