//! List admission and item continuation framing.
use crate::{
    Kind, MarkedOptions, Node,
    lexer::{Lexer, lines},
    rules,
    source::{ScopeSource, Source},
    tokenizer::{capture, node},
};
use maestro_text::{is_whitespace_scalar, truncate_utf16};
use std::ops::Range;

/// Marker identity and indentation at the start of one list item.
struct Marker {
    /// Ordered value, absent for bullets.
    start: Option<u64>,
    /// Delimiter or unordered bullet.
    bullet: u8,
    /// Complete first-line extent.
    line: usize,
    /// End of the marker before first-line whitespace.
    prefix: usize,
    /// Continuation indentation.
    indent: usize,
    /// Leading first-line tabs expanded before measuring content.
    tabs: usize,
}

impl Marker {
    /// Keep orderedness and the selected bullet or delimiter within one list.
    fn follows(&self, first: &Self) -> bool {
        self.start.is_some() == first.start.is_some() && self.bullet == first.bullet
    }
}

/// Recognize a marker without treating a horizontal rule as an item.
fn marker(source: &str) -> Option<Marker> {
    let matched = rules::LIST_ITEM.as_ref()?.find(source)?;
    if rules::BLOCK_HR.as_ref()?.find(source).is_some() {
        return None;
    }
    let raw = capture(source, &matched, 1);
    let bullet = *raw.as_bytes().last()?;
    let ordered = raw.trim_start().len() > 1;
    let start = ordered
        .then(|| raw.trim().trim_end_matches(['.', ')']).parse().ok())
        .flatten();
    let line = matched.end();
    let rest = &source[raw.len()..line];
    let blank = rest.trim_matches(is_whitespace_scalar).is_empty();
    let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
    let indent = if blank || spaces > 4 { 1 } else { spaces };
    let tabs = if blank {
        0
    } else {
        rest.bytes().take_while(|byte| *byte == b'\t').count()
    };
    Some(Marker {
        start,
        bullet,
        line,
        prefix: raw.len(),
        indent: raw.len() + indent,
        tabs,
    })
}

/// Item-local lookahead rules, prepared once for the selected indentation.
struct ItemRules {
    /// Fences, headings, neighboring markers, rules and HTML prefixes.
    stops: [Option<regress::Regex>; 5],
}
impl ItemRules {
    /// Bind the five shared prefix factories to this item's indentation.
    fn new(indent: usize, options: MarkedOptions) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.list_rules += 5);
        let prefix = format!("^ {{0,{}}}", indent.saturating_sub(1).min(3));
        let factories = [
            "(?:```|~~~)",
            "#",
            r"(?:[*+-]|\d{1,9}[.)])((?:[ \t][^\n]*)?(?:\n|$))",
            r"((?:- *){3,}|(?:_ *){3,}|(?:\* *){3,})(?:\n+|$)",
            if options == (MarkedOptions::Browser { escape_html: true }) {
                "<!--"
            } else {
                "<(?:[a-z].*>|!--)"
            },
        ];
        let stops =
            factories.map(|rule| regress::Regex::with_flags(&format!("{prefix}{rule}"), "i").ok());
        Self { stops }
    }

    /// A current line can end framing before it is dedented.
    fn interrupts(&self, line: &str) -> bool {
        self.stops
            .iter()
            .flatten()
            .any(|rule| rule.find(line).is_some())
    }

    /// A previous code line or block opener cannot admit another lazy line.
    fn ends_lazy(&self, line: &str) -> bool {
        let code = line
            .find(|scalar| !matches!(scalar, ' ' | '\t'))
            .is_some_and(|end| {
                line[..end]
                    .chars()
                    .map(|scalar| if scalar == '\t' { 4 } else { 1 })
                    .sum::<usize>()
                    >= 4
            });
        code || [0, 1, 3]
            .into_iter()
            .filter_map(|index| self.stops[index].as_ref())
            .any(|rule| rule.find(line).is_some())
    }
}

impl<E> Lexer<'_, E> {
    /// Capture each item's source before recursively admitting its blocks.
    pub(crate) fn list(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        rules::BLOCK_LIST.as_ref()?.find(source.text)?;
        let first = marker(source.text)?;
        let mut cursor = 0;
        let mut items = Vec::new();
        let mut loose = false;
        while let Some(current) = marker(&source.text[cursor..]) {
            if !current.follows(&first) {
                break;
            }
            let view = source.suffix(cursor);
            let (consumed, body) = item_lines(&view, &current, self.options);
            let (mut body, task) = task(body);
            let following =
                marker(&source.text[cursor + consumed..]).is_some_and(|next| next.follows(&first));
            if !following {
                body.truncate(body.text.trim_end_matches(is_whitespace_scalar).len());
            }
            let mut children = self.item_blocks(&body);
            if let Some(checked) = task {
                let task_node = node(&body, 0..0, Kind::TaskListMarker(checked));
                children.insert(0, task_node);
            }
            let raw = &source.text[cursor..cursor + consumed];
            loose |= rules::LIST_BLANK_END
                .as_ref()
                .is_some_and(|rule| rule.find(raw).is_some())
                && following
                || children.iter().any(|node| {
                    matches!(node.kind, Kind::Gap)
                        && node
                            .authored
                            .chunks()
                            .flat_map(str::bytes)
                            .filter(|byte| *byte == b'\n')
                            .count()
                            > 1
                });
            items.push(source.node(cursor..cursor + consumed, Kind::Item(children)));
            cursor += consumed;
            if cursor == source.text.len() {
                break;
            }
        }
        if items.is_empty() {
            return None;
        }
        let end = source.text[..cursor]
            .trim_end_matches(is_whitespace_scalar)
            .len();
        if loose {
            items.iter_mut().for_each(loosen);
        }
        Some((end, source.node(0..end, Kind::List(first.start, items))))
    }
}

/// Retained item framing, independent of emitted text and recursive block state.
struct ItemLines {
    /// Emitted spelling with its original source pieces.
    body: ScopeSource,
    /// Recognized raw prefix through the last admitted physical line.
    consumed: usize,
    /// Whether an admitted line has been blank.
    blank: bool,
    /// Whether the previous line prevents another lazy continuation.
    previous_stops: bool,
}
impl ItemLines {
    /// Append one eligible line using measured indentation and previous-line history.
    fn append(
        &mut self,
        source: &Source<'_>,
        range: Range<usize>,
        indent: usize,
        rules: &ItemRules,
    ) -> bool {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.list_lines += 1);
        let raw = &source.text[range.clone()];
        if rules.interrupts(raw) {
            return false;
        }
        let end = range.end - usize::from(raw.ends_with('\n'));
        let original = source.select(std::iter::once(range.start..end));
        let measured = original.replace("\t", "    ");
        let spaces = measured
            .text
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let empty = original.text.trim_matches(is_whitespace_scalar).is_empty();
        let aligned = spaces >= indent || empty;
        if !aligned && (self.blank || self.previous_stops) {
            return false;
        }
        let start = truncate_utf16(&measured.text, indent).len();
        self.body
            .append(source.select(std::iter::once(range.start - 1..range.start)));
        if aligned {
            self.body
                .append(measured.select(std::iter::once(start..measured.text.len())));
        } else {
            self.body.append(original);
        }
        self.previous_stops = rules.ends_lazy(&measured.text[start..]);
        self.blank |= empty;
        self.consumed = range.end;
        true
    }
}

/// Frame the first line before collecting continuations and their stopping history.
fn item_lines(
    source: &Source<'_>,
    marker: &Marker,
    options: MarkedOptions,
) -> (usize, ScopeSource) {
    let end = marker.line - usize::from(source.text[..marker.line].ends_with('\n'));
    let mut first = source.select(std::iter::once(marker.prefix..end));
    if marker.tabs > 0 {
        first = first.replace_prefix(marker.tabs, &"   ".repeat(marker.tabs));
    }
    let blank = first.text.trim_matches(is_whitespace_scalar).is_empty();
    let start = if blank {
        first.text.len()
    } else {
        marker.indent - marker.prefix
    };
    let body = first.select(std::iter::once(start..first.text.len()));
    if blank {
        let Some(next) = lines(&source.text[marker.line..]).next() else {
            return (marker.line, body);
        };
        let text = source.text[marker.line..marker.line + next.end].trim_end_matches('\n');
        if text.trim_matches([' ', '\t']).is_empty() {
            return (marker.line + next.end, body);
        }
    }
    let rules = ItemRules::new(marker.indent, options);
    let mut item = ItemLines {
        body,
        consumed: marker.line,
        blank,
        previous_stops: rules.ends_lazy(&first.text),
    };
    for range in lines(&source.text[marker.line..]) {
        let range = marker.line + range.start..marker.line + range.end;
        if !item.append(source, range, marker.indent, &rules) {
            break;
        }
    }
    (item.consumed, item.body)
}

/// Task markers are consumed before the item's ordinary inline grammar.
fn task(source: ScopeSource) -> (ScopeSource, Option<bool>) {
    let bytes = source.text.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'['
        && bytes[2] == b']'
        && bytes[3] == b' '
        && matches!(bytes[1], b' ' | b'x' | b'X')
    {
        let end = 3 + source.text[3..]
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let checked = bytes[1] != b' ';
        (
            source.select(std::iter::once(end..source.text.len())),
            Some(checked),
        )
    } else {
        (source, None)
    }
}

/// Loose items promote line text without changing genuine paragraphs.
fn loosen(item: &mut Node) {
    let Kind::Item(children) = &mut item.kind else {
        return;
    };
    for child in children {
        if let Kind::Inline(content) = &mut child.kind {
            child.kind = Kind::Paragraph(std::mem::take(content));
        }
    }
}
