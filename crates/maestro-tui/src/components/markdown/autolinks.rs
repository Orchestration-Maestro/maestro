//! Extended URL and email recognition on mapped native text runs.
use super::parse::{Kind, Node, Text};
use regress::Regex;
use std::{ops::Range, sync::LazyLock};

/// Web candidates, validated before reserving any text.
static WEB: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"(?:https?://|www\.)[^\s<]+").ok());
/// Email candidates scanned independently of rejected web spans.
static EMAIL: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9._+-]+@[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)+").ok());
/// Domain labels permit Unicode letters and numbers.
static DOMAIN: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::with_flags(r"^[\p{L}\p{N}_-]+(?:\.[\p{L}\p{N}_-]+)+", "u").ok());

/// Selects valid candidates before excluding overlaps with accepted links.
fn links(run: &Run) -> Vec<(Range<usize>, String)> {
    let mut candidates = Vec::new();
    for (regex, email) in [(WEB.as_ref(), false), (EMAIL.as_ref(), true)] {
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
/// Validates one web or email match and retains its authored spelling.
fn candidate(run: &Run, range: Range<usize>, email: bool) -> Option<(Range<usize>, String)> {
    let authored = &run.authored[range.clone()];
    let label = if email {
        if authored.ends_with(['-', '_']) {
            return None;
        }
        authored.trim_end_matches('.')
    } else {
        if run.authored[..range.start]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace() && !"*_~(".contains(c))
        {
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
    Some((range.start..range.start + label.len(), href))
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
    fn new(parts: Vec<Node>) -> Self {
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
                run.spans(part.range, decoded, authored);
            }
        }
        run
    }
    /// Splits native text into equal spans and individual escaped punctuation.
    /// A differing remainder is a native entity event, kept atomic.
    fn spans(&mut self, source: Range<usize>, mut decoded: usize, mut authored: usize) {
        while decoded < self.decoded.len() {
            let display = &self.decoded[decoded..];
            let literal = &self.authored[authored..];
            let equal = display
                .chars()
                .zip(literal.chars())
                .take_while(|(left, right)| left == right)
                .map(|(left, _)| left.len_utf8())
                .sum::<usize>();
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
            self.units.push(Unit {
                source: source.clone(),
                decoded: decoded..decoded + display_len,
                authored: authored..authored + literal_len,
            });
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
        let start = self
            .units
            .iter()
            .find(|unit| unit.authored.end > range.start)
            .map_or(0, |unit| unit.source.start);
        let end = self
            .units
            .iter()
            .rev()
            .find(|unit| unit.authored.start < range.end)
            .map_or(start, |unit| unit.source.end);
        start..end
    }
    /// Constructs one surrounding text child without unused link spellings.
    fn node(&self, range: Range<usize>) -> Node {
        Node {
            range: self.source_range(range.clone()),
            kind: Kind::Text(self.text(range)),
        }
    }
}

/// Replaces accepted links while preserving native-decoded surrounding text.
pub(super) fn extend(parts: Vec<Node>) -> Vec<Node> {
    if parts.is_empty() {
        return parts;
    }
    let run = Run::new(parts);
    let mut output = Vec::new();
    let mut end = 0;
    for (range, href) in links(&run) {
        if range.start > end {
            output.push(run.node(end..range.start));
        }
        let label = run.authored[range.clone()].to_owned();
        let absolute = run.source_range(range.clone());
        let child = Node {
            range: absolute.clone(),
            kind: Kind::Text(Text {
                decoded: label.clone(),
                authored: label.clone(),
            }),
        };
        output.push(Node {
            kind: Kind::Link(vec![child], label, href),
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
    let text = label
        .strip_prefix("https://")
        .or_else(|| label.strip_prefix("http://"))
        .or_else(|| label.strip_prefix("www."))
        .unwrap_or(label);
    regex.find(text).is_some_and(|domain| {
        !text[domain.range()]
            .rsplit('.')
            .take(2)
            .any(|part| part.contains('_'))
    })
}
