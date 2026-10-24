//! Paste-marker grammar, ownership and marker-aware segmentation.
use super::wrapping::{Atom, atoms};
use crate::get_segmenter;
use std::ops::Range;

/// Opening of every marker spelling.
const PREFIX: &str = "[paste #";

/// One marker spelling found in text.
pub(super) struct Marker<'a> {
    /// Byte range of the whole spelling.
    pub(super) range: Range<usize>,
    /// ASCII digits of the identifier, exactly as written.
    pub(super) id: &'a str,
}

/// Finds marker spellings from left to right without overlap.
pub(super) fn find(text: &str) -> Vec<Marker<'_>> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find(PREFIX) {
        let at = from + offset;
        if let Some((length, id)) = parse(&text[at..]) {
            found.push(Marker {
                range: at..at + length,
                id,
            });
            from = at + length;
        } else {
            from = at + 1;
        }
    }
    found
}

/// Reads one spelling at the start of `text`: byte length and identifier digits.
fn parse(text: &str) -> Option<(usize, &str)> {
    let body = &text[PREFIX.len()..];
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    let (id, rest) = body.split_at(digits);
    if id.is_empty() {
        return None;
    }
    let tail = if rest.starts_with(']') {
        1
    } else {
        label_length(rest)?
    };
    Some((PREFIX.len() + digits + tail, id))
}

/// Length of ` +N lines]` or ` N chars]` at the start of `rest`.
fn label_length(rest: &str) -> Option<usize> {
    let body = rest.strip_prefix(' ')?;
    let (plus, body) = body
        .strip_prefix('+')
        .map_or((false, body), |body| (true, body));
    let digits = body.bytes().take_while(u8::is_ascii_digit).count();
    let word = if plus { " lines]" } else { " chars]" };
    (digits > 0 && body[digits..].starts_with(word))
        .then_some(1 + usize::from(plus) + digits + word.len())
}

/// Whether `text` holds a marker spelling and no whitespace outside its first spelling.
pub(super) fn is_marker(text: &str) -> bool {
    text.contains(PREFIX)
        && find(text).first().is_some_and(|marker| {
            !text[..marker.range.start]
                .chars()
                .chain(text[marker.range.end..].chars())
                .any(crate::text::utils::is_whitespace_scalar)
        })
}

/// Byte ranges of markers whose identifier names one of the first `owned` stored pastes.
fn owned_spans(text: &str, owned: usize) -> Vec<Range<usize>> {
    if owned == 0 || !text.contains(PREFIX) {
        return Vec::new();
    }
    find(text)
        .into_iter()
        .filter(|marker| {
            marker
                .id
                .parse::<usize>()
                .is_ok_and(|id| (1..=owned).contains(&id))
        })
        .map(|marker| marker.range)
        .collect()
}

/// Graphemes of `text` where every owned marker and the graphemes it intersects form one segment.
pub(super) fn segments(text: &str, owned: usize) -> Vec<(usize, &str)> {
    let spans = owned_spans(text, owned);
    let mut result: Vec<(usize, &str)> = Vec::new();
    let mut index = 0;
    let mut joined = None;
    for (at, grapheme) in get_segmenter(text) {
        let end = at + grapheme.len();
        while spans.get(index).is_some_and(|span| span.end <= at) {
            index += 1;
        }
        let hit = spans.get(index).is_some_and(|span| span.start < end);
        match result.last_mut() {
            Some(last) if hit && joined == Some(index) => last.1 = &text[last.0..end],
            _ => {
                joined = hit.then_some(index);
                result.push((at, grapheme));
            }
        }
    }
    result
}

/// Visible atoms of `line` where every owned marker's atoms form one unit.
pub(super) fn units(line: &str, owned: usize) -> Vec<Atom> {
    let spans = owned_spans(line, owned);
    let mut result: Vec<Atom> = Vec::new();
    let mut index = 0;
    let mut joined = None;
    for atom in atoms(line) {
        while spans.get(index).is_some_and(|span| span.end <= atom.start) {
            index += 1;
        }
        let hit = spans.get(index).is_some_and(|span| span.start < atom.end);
        match result.last_mut() {
            Some(last) if hit && joined == Some(index) => {
                last.end = atom.end;
                last.cells += atom.cells;
            }
            _ => {
                joined = hit.then_some(index);
                result.push(atom);
            }
        }
    }
    result
}

/// One literal replacement pass: each canonical spelling of paste `id` becomes `content`.
pub(super) fn expand(text: &str, id: usize, content: &str) -> String {
    let id = id.to_string();
    let mut result = String::with_capacity(text.len());
    let mut from = 0;
    for marker in find(text).into_iter().filter(|marker| marker.id == id) {
        result.push_str(&text[from..marker.range.start]);
        result.push_str(content);
        from = marker.range.end;
    }
    result.push_str(&text[from..]);
    result
}
