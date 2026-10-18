//! List admission and item continuation framing.
use crate::{
    Kind, MarkedOptions, Node,
    lexer::{Lexer, lines},
    rules,
    source::{ScopeSource, Source},
    tokenizer::{capture, node},
};
use maestro_text::is_whitespace_scalar;

/// Marker identity and indentation at the start of one list item.
struct Marker {
    /// Ordered value, absent for bullets.
    start: Option<u64>,
    /// Delimiter or unordered bullet.
    bullet: u8,
    /// Complete first-line extent.
    line: usize,
    /// Content start after eligible marker whitespace.
    content: usize,
    /// Continuation indentation.
    indent: usize,
}

impl Marker {
    /// Keep orderedness and the selected bullet or delimiter within one list.
    fn follows(&self, first: &Self) -> bool {
        self.start.is_some() == first.start.is_some() && self.bullet == first.bullet
    }
}

/// Recognize a marker without treating a horizontal rule as an item.
fn marker(source: &str) -> Option<Marker> {
    let matched = rules::BLOCK_LIST.as_ref()?.find(source)?;
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
    let spaces = rest
        .bytes()
        .take_while(|byte| *byte == b' ' || *byte == b'\t')
        .count();
    let indent = if (1..=4).contains(&spaces) { spaces } else { 1 };
    let content = if rest.trim_matches(is_whitespace_scalar).is_empty() {
        line
    } else {
        raw.len() + indent
    };
    Some(Marker {
        start,
        bullet,
        line,
        content,
        indent: raw.len() + indent,
    })
}

/// A continuation can stop at another block admitted before the item indentation.
fn interrupts(line: &str, indent: usize, options: MarkedOptions) -> bool {
    let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
    if spaces >= indent.min(4) {
        return false;
    }
    let line = &line[spaces..];
    line.starts_with("```")
        || line.starts_with("~~~")
        || line.starts_with('#')
        || marker(line).is_some()
        || rules::BLOCK_HR
            .as_ref()
            .is_some_and(|rule| rule.find(line).is_some())
        || (options != (MarkedOptions::Browser { escape_html: true })
            && line.strip_prefix('<').is_some_and(|text| {
                text.starts_with("!--")
                    || text
                        .bytes()
                        .next()
                        .is_some_and(|byte| byte.is_ascii_alphabetic())
                        && text.contains('>')
            }))
}

impl<E> Lexer<'_, E> {
    /// Capture each item's source before recursively admitting its blocks.
    pub(crate) fn list(&mut self, source: &Source<'_>) -> Option<(usize, Node)> {
        let first = marker(source.text)?;
        let mut cursor = 0;
        let mut items = Vec::new();
        let mut loose = false;
        while let Some(current) = marker(&source.text[cursor..]) {
            if !current.follows(&first) {
                break;
            }
            let (consumed, pieces, lazy) =
                item_lines(&source.text[cursor..], &current, self.options);
            let pieces = pieces
                .into_iter()
                .map(|range| cursor + range.start..cursor + range.end);
            let mut body = source.select(pieces);
            body.lazy_lines = lazy;
            let (body, task) = task(body);
            let following =
                marker(&source.text[cursor + consumed..]).is_some_and(|next| next.follows(&first));
            let end = if following {
                body.text.trim_end_matches('\n').len()
            } else {
                body.text.trim_end_matches(is_whitespace_scalar).len()
            };
            let body = body.select(std::iter::once(0..end));
            let mut children = self.item_blocks(&body);
            if let Some(checked) = task {
                let task_node = node(&body, 0..0, Kind::TaskListMarker(checked));
                children.insert(0, task_node);
            }
            let raw = &source.text[cursor..cursor + consumed];
            loose |=
                raw.trim_end_matches('\n').contains("\n\n") || raw.ends_with("\n\n") && following;
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
        if !loose {
            items.iter_mut().for_each(tighten);
        }
        Some((end, source.node(0..end, Kind::List(first.start, items))))
    }
}

/// Project a first line and its eligible continuations into one item body.
fn item_lines(
    source: &str,
    marker: &Marker,
    options: MarkedOptions,
) -> (usize, Vec<std::ops::Range<usize>>, Vec<usize>) {
    let mut pieces = std::iter::once(marker.content..marker.line).collect::<Vec<_>>();
    let mut consumed = marker.line;
    let mut lazy = Vec::new();
    let mut local = marker.line - marker.content;
    let mut blank = source[marker.content..marker.line]
        .trim_matches(is_whitespace_scalar)
        .is_empty();
    for range in lines(&source[marker.line..]) {
        let range = marker.line + range.start..marker.line + range.end;
        let line = &source[range.clone()];
        if interrupts(line, marker.indent, options) {
            break;
        }
        let spaces = line.bytes().take_while(|byte| *byte == b' ').count();
        let empty = line.trim_matches(is_whitespace_scalar).is_empty();
        if spaces < marker.indent && blank && !empty {
            break;
        }
        let strip = if spaces >= marker.indent {
            marker.indent
        } else {
            0
        };
        if strip == 0 && !empty {
            lazy.push(local);
        }
        local += range.len() - strip;
        pieces.push(range.start + strip..range.end);
        consumed = range.end;
        blank |= empty;
    }
    (consumed, pieces, lazy)
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

/// Tight items expose inline containers instead of paragraph wrappers.
fn tighten(item: &mut Node) {
    let Kind::Item(children) = &mut item.kind else {
        return;
    };
    for child in children {
        if let Kind::Paragraph(content) = &mut child.kind {
            child.kind = Kind::Inline(std::mem::take(content));
        }
    }
}
