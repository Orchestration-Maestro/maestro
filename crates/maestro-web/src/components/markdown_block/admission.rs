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

/// Native code and angle autolink extents, in source order and without overlap.
fn literal(document: &Document) -> Vec<Range<usize>> {
    let mut spans: Vec<Range<usize>> = Vec::new();
    visit(&document.nodes, &mut |node| {
        let native = match &node.kind {
            Kind::Code(_) | Kind::CodeBlock(..) => true,
            Kind::Link(_, _, _, meta) => {
                matches!(meta.link_type, LinkType::Autolink | LinkType::Email)
                    && !authored_label(document, node)
            }
            _ => false,
        };
        if native && spans.last().is_none_or(|last| last.end <= node.range.start) {
            spans.push(node.range.clone());
        }
    });
    spans
}

/// Escape tag-like text outside native code and angle autolinks, and parse the result.
///
/// Escaping can change the structure around a span, so a span stays literal only while
/// it is still native code or an autolink at the same place in the escaped document.
/// The first span that is not, and every later span, whose structure may follow from
/// it, are escaped too.
pub(super) fn escape_tags(source: String) -> Document {
    let first = markdown::parse(source);
    let mut kept = literal(&first);
    let source = first.source;
    loop {
        let (text, moved) = shown_outside(&source, &kept);
        let document = markdown::parse(text);
        let native = literal(&document);
        let lost = moved.iter().position(|at| {
            !native
                .binary_search_by_key(&at.start, |span| span.start)
                .is_ok_and(|index| native[index] == *at)
        });
        let Some(lost) = lost else {
            return document;
        };
        kept.truncate(lost);
    }
}

/// `source` with tag-like text outside `spans` shown as text, and where each span lands.
fn shown_outside(source: &str, spans: &[Range<usize>]) -> (String, Vec<Range<usize>>) {
    let mut out = String::with_capacity(source.len());
    let mut moved = Vec::with_capacity(spans.len());
    let mut cursor = 0;
    for span in spans {
        out.push_str(&shown_as_text(&source[cursor..span.start]));
        let start = out.len();
        out.push_str(&source[span.clone()]);
        moved.push(start..out.len());
        cursor = span.end;
    }
    out.push_str(&shown_as_text(&source[cursor..]));
    (out, moved)
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

/// Formulas of one display class, using original mapped operands without reparsing.
pub(super) fn scan(
    scope: &markdown::InlineScope<'_>,
    range: Range<usize>,
    display: bool,
) -> Vec<Formula> {
    let source = &scope.text()[..range.end];
    if !source[range.start..].contains(['$', '\\']) {
        return Vec::new();
    }
    let opaque = &scope.operands;
    let (mut found, mut next, mut slashes) = (
        Vec::new(),
        opaque.partition_point(|(span, _)| span.end <= range.start),
        0_usize,
    );
    let mut index = range.start;
    while index < source.len() {
        while opaque
            .get(next)
            .is_some_and(|(range, _)| range.end <= index)
        {
            next += 1;
        }
        if let Some((range, _)) = opaque.get(next).filter(|(r, _)| r.contains(&index)) {
            index = range.end;
            slashes = 0;
            continue;
        }
        let formula = (slashes % 2 == 0)
            .then(|| matching(source, index, display))
            .flatten();
        let barrier = scope.opaque.partition_point(|range| range.end <= index);
        if let Some(formula) = formula.filter(|formula| {
            scope
                .opaque
                .get(barrier)
                .is_none_or(|range| range.start >= formula.range.end)
        }) {
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
