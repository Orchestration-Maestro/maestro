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
        if let Some(regex) = regex {
            candidates.extend(
                regex
                    .find_iter(&run.decoded)
                    .filter_map(|matched| candidate(run, matched.range(), email)),
            );
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
    let authored = run.authored(range.clone());
    let label = if email {
        if authored.ends_with(['-', '_']) {
            return None;
        }
        authored.trim_end_matches('.')
    } else {
        if run.decoded[..range.start]
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
    let start = run.authored_offset(range.start);
    Some((
        run.decoded_offset(start)..run.decoded_offset(start + label.len()),
        href,
    ))
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

/// One contiguous native text run with decoded-to-authored source units.
struct Run {
    /// Text used for recognition and display.
    decoded: String,
    /// Text used for selected literal link spelling.
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
    /// Maps boundaries linearly in equal spans, snapping only atomic transforms.
    fn authored_offset(&self, offset: usize) -> usize {
        self.units
            .iter()
            .find(|unit| unit.decoded.end > offset)
            .map_or(self.authored.len(), |unit| {
                unit.authored.start
                    + if self.decoded[unit.decoded.clone()] == self.authored[unit.authored.clone()]
                    {
                        offset - unit.decoded.start
                    } else {
                        0
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
    /// Borrows literal text without copying the unused display content.
    fn authored(&self, range: Range<usize>) -> &str {
        &self.authored[self.authored_offset(range.start)..self.authored_offset(range.end)]
    }
    /// Selects display and literal text from the same mapped interval.
    fn text(&self, range: Range<usize>) -> Text {
        Text {
            authored: self.authored(range.clone()).to_owned(),
            decoded: self.decoded[range].to_owned(),
        }
    }
    /// Preserves the source extent of selected native text units.
    fn node(&self, range: Range<usize>) -> Node {
        let start = self
            .units
            .iter()
            .find(|unit| unit.decoded.end > range.start)
            .map_or(0, |unit| unit.source.start);
        let end = self
            .units
            .iter()
            .rev()
            .find(|unit| unit.decoded.start < range.end)
            .map_or(start, |unit| unit.source.end);
        Node {
            kind: Kind::Text(self.text(range)),
            range: start..end,
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
        let label = run.authored(range.clone()).to_owned();
        let mut child = run.node(range.clone());
        child.kind = Kind::Text(Text {
            decoded: label.clone(),
            authored: label.clone(),
        });
        let absolute = child.range.clone();
        output.push(Node {
            kind: Kind::Link(vec![child], label, href),
            range: absolute,
        });
        end = range.end;
    }
    if end < run.decoded.len() {
        output.push(run.node(end..run.decoded.len()));
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

#[cfg(test)]
mod tests;
