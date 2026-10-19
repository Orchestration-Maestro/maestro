//! Extended URL and email recognition on mapped native text runs.
use crate::{
    inline::ScopeKey,
    source::ScopeSource,
    types::{Document, Kind, MarkedOptions, Node, Text},
};
use regress::Regex;
use std::{collections::HashMap, ops::Range, sync::LazyLock};

/// Web, earlier-grammar web and email candidate sources with their flags; earlier caller
/// grammars fold the complete protocol and web prefix, and emails are scanned
/// independently of rejected web spans.
const SOURCES: [(&str, &str); 3] = [
    (
        r"(?:(?:[hH][tT][tT][pP][sS]?|[fF][tT][pP])://|www\.)[^\s<]+",
        "",
    ),
    (r"(?:(?:https?|ftp)://|www\.)[^\s<]+", "i"),
    (r"[A-Za-z0-9._+-]+@[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+", ""),
];
/// Candidate rules for scanning a text run, validated before reserving any text.
static SCAN: LazyLock<[Option<Regex>; 3]> =
    LazyLock::new(|| SOURCES.map(|(source, flags)| Regex::with_flags(source, flags).ok()));
/// The same rules anchored at an inline token start.
static START: LazyLock<[Option<Regex>; 3]> = LazyLock::new(|| {
    SOURCES.map(|(source, flags)| Regex::with_flags(&format!("^(?:{source})"), flags).ok())
});
/// Domain labels permit Unicode letters and numbers.
static DOMAIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^[\p{L}\p{N}_-]+(?:\.[\p{L}\p{N}_-]+)+", "u").ok());

/// Selects valid candidates before excluding overlaps with accepted links.
fn links(run: &Run, options: MarkedOptions) -> Vec<(Range<usize>, String)> {
    let mut candidates = Vec::new();
    for (regex, email) in rules(options, &SCAN) {
        let Some(regex) = regex else {
            continue;
        };
        let mut offset = 0;
        while let Some(matched) = regex.find_from(&run.authored, offset).next() {
            let range = matched.range();
            let start = run.recognition_start(range.start);
            if start != range.start {
                offset = start;
                continue;
            }
            offset = range.end;
            candidates.extend(candidate(run, range, email));
        }
    }
    candidates.sort_by_key(|(range, _)| range.start);
    let mut end = 0;
    candidates
        .into_iter()
        .filter(|(range, _)| {
            if range.start < end {
                false
            } else {
                end = range.end;
                true
            }
        })
        .collect()
}
/// The grammar's web rule and the email rule from one rule set.
fn rules(
    options: MarkedOptions,
    set: &'static [Option<Regex>; 3],
) -> [(Option<&'static Regex>, bool); 2] {
    let web = usize::from(matches!(
        options,
        MarkedOptions::Terminal | MarkedOptions::Export
    ));
    [(set[web].as_ref(), false), (set[2].as_ref(), true)]
}

/// Length of the web or email address the run scan would link at the start of `text`.
///
/// `previous` is the character before it in the same text run.
pub(crate) fn address(text: &str, previous: Option<char>, options: MarkedOptions) -> Option<usize> {
    rules(options, &START)
        .into_iter()
        .find_map(|(regex, email)| {
            // The run scan starts an email where its run of address characters starts.
            let inside = |c: char| c.is_ascii_alphanumeric() || "._+-".contains(c);
            if email && previous.is_some_and(inside) {
                return None;
            }
            let matched = regex?.find(text)?;
            Some(accepted(previous, &text[matched.range()], email)?.0)
        })
}

/// Validates one web or email match and retains its authored spelling.
fn candidate(run: &Run, range: Range<usize>, email: bool) -> Option<(Range<usize>, String)> {
    let previous = run.authored[..range.start].chars().next_back();
    let (length, href) = accepted(previous, &run.authored[range.clone()], email)?;
    Some((range.start..range.start + length, href))
}

/// Accept a candidate after `previous`, returning its label length and destination.
fn accepted(previous: Option<char>, authored: &str, email: bool) -> Option<(usize, String)> {
    let label = if email {
        if authored.ends_with(['-', '_']) {
            return None;
        }
        authored.trim_end_matches('.')
    } else {
        if previous.is_some_and(|c| !maestro_text::is_whitespace_scalar(c) && !"*_~(".contains(c)) {
            return None;
        }
        let label = suffix(authored);
        if !domain(label) {
            return None;
        }
        label
    };
    let href = if email {
        format!("mailto:{label}")
    } else if label.starts_with("www.") {
        format!("http://{label}")
    } else {
        label.to_owned()
    };
    Some((label.len(), href))
}
/// Reduces punctuation, unbalanced closing parentheses and entity tails.
fn suffix(candidate: &str) -> &str {
    let mut label = candidate;
    loop {
        let previous = label.len();
        label = label.trim_end_matches(['?', '!', '.', ',', ':', '*', '_', '~']);
        let opens = label.matches('(').count();
        let mut closes = label.matches(')').count();
        while label.ends_with(')') && closes > opens {
            label = &label[..label.len() - 1];
            closes -= 1;
        }
        if label.ends_with(';')
            && let Some((head, tail)) = label.rsplit_once('&')
            && tail[..tail.len() - 1]
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
            && tail.len() > 1
        {
            label = head;
        }
        if label.len() == previous {
            return label;
        }
    }
}

/// One contiguous authored text run with mapped native display units.
struct Run {
    /// Native-decoded text used outside selected links.
    decoded: String,
    /// Authored text used for recognition and selected literal link spelling.
    authored: String,
    /// Ordered spans with decoded, authored and original source extents.
    units: Vec<Unit>,
}
/// Buffer extents of an equal span or one atomic native transformation.
struct Unit {
    /// Original normalized source extent.
    source: Range<usize>,
    /// Decoded buffer extent.
    decoded: Range<usize>,
    /// Authored buffer extent.
    authored: Range<usize>,
}
impl Run {
    /// Builds ordered buffers without container source bytes.
    fn new(parts: Vec<Node>, mapping: Option<&ScopeSource>) -> Self {
        let mut run = Self {
            decoded: String::new(),
            authored: String::new(),
            units: Vec::new(),
        };
        for part in parts {
            if let Kind::Text(text) = &part.kind {
                let decoded = run.decoded.len();
                let authored = run.authored.len();
                run.decoded.push_str(&text.decoded);
                run.authored.push_str(&text.authored);
                run.spans(part.range, (decoded, authored), mapping);
            }
        }
        run
    }
    /// Splits native text into equal spans and individual escaped punctuation.
    /// A differing remainder is a native entity event, kept atomic.
    fn spans(
        &mut self,
        mut source: Range<usize>,
        (mut decoded, mut authored): (usize, usize),
        mapping: Option<&ScopeSource>,
    ) {
        let base = authored;
        let local = mapping
            .and_then(|mapping| mapping.local(&source))
            .map(|range| range.start);
        while decoded < self.decoded.len() {
            let display = &self.decoded[decoded..];
            let literal = &self.authored[authored..];
            let mut equal = display
                .chars()
                .zip(literal.chars())
                .take_while(|(left, right)| left == right)
                .map(|(left, _)| left.len_utf8())
                .sum::<usize>();
            if let Some((mapping, local)) = mapping.zip(local) {
                equal = equal.min(mapping.remaining(local + authored - base));
            }
            let (display_len, literal_len) = if literal.starts_with('\\')
                && display.as_bytes()[0].is_ascii_punctuation()
                && literal.as_bytes().get(1) == display.as_bytes().first()
            {
                (1, 2)
            } else if equal > 0 && (!literal.starts_with('&') || display == literal) {
                (equal, equal)
            } else {
                (display.len(), literal.len())
            };
            let end = if literal_len == literal.len() {
                source.end
            } else {
                source.start + literal_len
            };
            let selected = mapping.zip(local).and_then(|(mapping, local)| {
                mapping.native(&(local + authored - base..local + authored - base + literal_len))
            });
            let selected = selected.unwrap_or(source.start..end);
            let next = selected.end;
            self.units.push(Unit {
                source: selected,
                decoded: decoded..decoded + display_len,
                authored: authored..authored + literal_len,
            });
            source.start = next;
            decoded += display_len;
            authored += literal_len;
        }
    }
    /// Advances a candidate start past punctuation already consumed as an escape.
    fn recognition_start(&self, offset: usize) -> usize {
        let index = self
            .units
            .partition_point(|unit| unit.authored.end <= offset);
        self.units.get(index).map_or(offset, |unit| {
            if unit.authored.start < offset
                && unit.authored.len() == 2
                && unit.decoded.len() == 1
                && self.authored.as_bytes()[unit.authored.start] == b'\\'
            {
                unit.authored.end
            } else {
                offset
            }
        })
    }
    /// Maps literal boundaries through the same ordered spans as display text.
    fn decoded_offset(&self, offset: usize) -> usize {
        self.units
            .iter()
            .find(|unit| unit.authored.end > offset)
            .map_or(self.decoded.len(), |unit| {
                unit.decoded.start
                    + if self.decoded[unit.decoded.clone()] == self.authored[unit.authored.clone()]
                    {
                        offset - unit.authored.start
                    } else {
                        0
                    }
            })
    }
    /// Selects literal boundaries and the corresponding decoded display text.
    fn text(&self, range: Range<usize>) -> Text {
        Text {
            decoded: self.decoded[self.decoded_offset(range.start)..self.decoded_offset(range.end)]
                .to_owned(),
            authored: self.authored[range].to_owned(),
        }
    }
    /// Preserves the source extent of selected native text units.
    fn source_range(&self, range: Range<usize>) -> Range<usize> {
        let first = self.units.partition_point(|unit| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_examinations += 1);
            unit.authored.end <= range.start
        });
        let start = self.units.get(first).map_or(0, |unit| {
            if unit.source.len() == unit.authored.len() {
                unit.source.start + range.start - unit.authored.start
            } else {
                unit.source.start
            }
        });
        let last = self.units.partition_point(|unit| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_examinations += 1);
            unit.authored.start < range.end
        });
        let end = last
            .checked_sub(1)
            .and_then(|index| self.units.get(index))
            .map_or(start, |unit| {
                if unit.source.len() == unit.authored.len() {
                    unit.source.start + range.end - unit.authored.start
                } else {
                    unit.source.end
                }
            });
        start..end
    }
    /// Constructs one surrounding text child without unused link spellings.
    fn node(&self, range: Range<usize>) -> Node {
        Node {
            authored: self.authored[range.clone()].to_owned(),
            range: self.source_range(range.clone()),
            kind: Kind::Text(self.text(range)),
        }
    }
}

/// Replaces accepted links while preserving native-decoded surrounding text.
fn extend(parts: Vec<Node>, mapping: Option<&ScopeSource>, options: MarkedOptions) -> Vec<Node> {
    if parts.is_empty() {
        return parts;
    }
    let run = Run::new(parts, mapping);
    let mut output = Vec::new();
    let mut end = 0;
    for (range, href) in links(&run, options) {
        if range.start > end {
            output.push(run.node(end..range.start));
        }
        let label = run.authored[range.clone()].to_owned();
        let absolute = run.source_range(range.clone());
        let child = Node {
            authored: label.clone(),
            range: absolute.clone(),
            kind: Kind::Text(Text {
                decoded: label.clone(),
                authored: label.clone(),
            }),
        };
        output.push(Node {
            authored: label.clone(),
            kind: Kind::Link(
                vec![child],
                label,
                href,
                crate::types::LinkMetadata {
                    title: String::new(),
                    link_type: crate::LinkType::Autolink,
                    id: String::new(),
                },
            ),
            range: absolute,
        });
        end = range.end;
    }
    if end < run.authored.len() {
        output.push(run.node(end..run.authored.len()));
    }
    output
}
/// Checks domain labels and the final two underscore-free segments.
fn domain(label: &str) -> bool {
    let Some(regex) = DOMAIN.as_ref() else {
        return false;
    };
    let text = label.split_once("://").map_or_else(
        || label.strip_prefix("www.").unwrap_or(label),
        |(_, text)| text,
    );
    regex.find(text).is_some_and(|domain| {
        !text[domain.range()]
            .rsplit('.')
            .take(2)
            .any(|part| part.contains('_'))
    })
}

/// Apply shared mapped extended links after caller-owned inline admission.
pub fn autolinks(document: &mut Document) {
    map_links(
        &mut document.nodes,
        &document.scopes,
        None,
        document.options,
        &mut false,
    );
}

/// Scan eligible runs in source order, retaining raw-anchor state across containers.
fn map_links(
    nodes: &mut Vec<Node>,
    scopes: &HashMap<ScopeKey, ScopeSource>,
    mapping: Option<&ScopeSource>,
    options: MarkedOptions,
    in_link: &mut bool,
) {
    let mut result = Vec::new();
    let mut parts = Vec::new();
    for mut node in std::mem::take(nodes) {
        if matches!(node.kind, Kind::Text(_)) && !*in_link {
            parts.push(node);
            continue;
        }
        result.extend(extend(std::mem::take(&mut parts), mapping, options));
        map_children(&mut node, scopes, mapping, options, in_link);
        result.push(node);
    }
    result.extend(extend(parts, mapping, options));
    *nodes = result;
}

/// Reuse the original scope mapping through formatted spans and table cells.
fn map_children(
    node: &mut Node,
    scopes: &HashMap<ScopeKey, ScopeSource>,
    mapping: Option<&ScopeSource>,
    options: MarkedOptions,
    in_link: &mut bool,
) {
    let mapping = scopes
        .get(&(node.range.start, node.range.end, false))
        .or(mapping);
    match &mut node.kind {
        Kind::Paragraph(children)
        | Kind::Inline(children)
        | Kind::Strong(children)
        | Kind::Strike(children)
        | Kind::Emphasis(children)
        | Kind::Item(children)
        | Kind::List(_, children)
        | Kind::Heading(_, children, _)
        | Kind::Quote(children, _) => map_links(children, scopes, mapping, options, in_link),
        Kind::Table(table) => {
            for cell in table
                .header
                .iter_mut()
                .chain(table.rows.iter_mut().flatten())
            {
                map_links(
                    &mut cell.children,
                    scopes,
                    scopes.get(&(cell.range.start, cell.range.end, false)),
                    options,
                    in_link,
                );
            }
        }
        Kind::Html(text) => *in_link = anchor_state(text, *in_link),
        _ => {}
    }
}

/// Follow the grammar's raw opening and closing anchor transitions.
fn anchor_state(text: &str, in_link: bool) -> bool {
    if text
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("<a "))
    {
        true
    } else if text
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("</a>"))
    {
        false
    } else {
        in_link
    }
}
