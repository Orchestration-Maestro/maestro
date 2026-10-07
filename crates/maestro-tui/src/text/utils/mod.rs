//! ANSI-aware terminal cell operations.

use std::{cell::RefCell, collections::VecDeque};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

thread_local! {
    static WIDTH_CACHE: RefCell<VecDeque<(String, usize)>> = const { RefCell::new(VecDeque::new()) };
}

fn is_printable_ascii(text: &str) -> bool {
    text.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
}

fn grapheme_width(grapheme: &str) -> usize {
    // A partial flag occupies the same cells as the completed flag while streaming.
    if grapheme
        .chars()
        .find(|ch| ch.width().unwrap_or(0) > 0)
        .is_some_and(|ch| ('🇦'..='🇿').contains(&ch))
    {
        2
    } else {
        UnicodeWidthStr::width(grapheme)
    }
}

fn ansi_len(text: &str) -> Option<usize> {
    let body = text.strip_prefix("\x1b")?;
    match body.as_bytes().first()? {
        b'[' => body
            .bytes()
            .position(|byte| b"mGKHJ".contains(&byte))
            .map(|offset| offset + 2),
        b']' | b'_' => body.char_indices().find_map(|(offset, ch)| match ch {
            '\x07' => Some(offset + 2),
            '\x1b' if body[offset..].starts_with("\x1b\\") => Some(offset + 3),
            _ => None,
        }),
        _ => None,
    }
}
enum TextPiece<'a> {
    Ansi(&'a str),
    Text(&'a str),
}
fn fragments(mut text: &str) -> impl Iterator<Item = TextPiece<'_>> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        if let Some(length) = ansi_len(text) {
            let (code, rest) = text.split_at(length);
            text = rest;
            return Some(TextPiece::Ansi(code));
        }
        let end = text
            .char_indices()
            .skip(1)
            .find(|(offset, ch)| *ch == '\x1b' && ansi_len(&text[*offset..]).is_some())
            .map_or(text.len(), |(offset, _)| offset);
        let (plain, rest) = text.split_at(end);
        text = rest;
        Some(TextPiece::Text(plain))
    })
}
fn pieces(text: &str) -> impl Iterator<Item = TextPiece<'_>> {
    fragments(text).flat_map(|piece| {
        let (ansi, plain) = match piece {
            TextPiece::Ansi(code) => (Some(TextPiece::Ansi(code)), ""),
            TextPiece::Text(plain) => (None, plain),
        };
        ansi.into_iter()
            .chain(plain.graphemes(true).map(TextPiece::Text))
    })
}
/// Measure visible terminal columns; tabs occupy three cells.
#[must_use]
pub fn visible_width(text: &str) -> usize {
    if is_printable_ascii(text) {
        return text.len();
    }
    if let Some(width) = WIDTH_CACHE.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(key, _)| key == text)
            .map(|(_, width)| *width)
    }) {
        return width;
    }
    // Strip escapes before segmentation so they cannot split a visible cluster.
    let clean: String = fragments(text)
        .filter_map(|piece| match piece {
            TextPiece::Ansi(_) => None,
            TextPiece::Text(plain) => Some(plain),
        })
        .collect::<String>()
        .replace('\t', "   ");
    let width = clean.graphemes(true).map(grapheme_width).sum();
    WIDTH_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= 512 {
            cache.pop_front();
        }
        cache.push_back((text.to_owned(), width));
    });
    width
}
/// Decompose only Thai and Lao AM vowels for terminal output.
#[must_use]
pub fn normalize_terminal_output(text: &str) -> String {
    text.replace('\u{e33}', "\u{e4d}\u{e32}")
        .replace('\u{eb3}', "\u{ecd}\u{eb2}")
}

/// Shared grapheme segmenter with owned-interface results.
pub struct Segmenter;
/// A grapheme and its byte location in the original input.
pub struct SegmentData<'a> {
    /// Grapheme text.
    pub segment: &'a str,
    /// Start offset in UTF-8 bytes.
    pub index: usize,
    /// Original input.
    pub input: &'a str,
}
impl Segmenter {
    /// Iterate graphemes, retaining their byte offsets.
    pub fn segment<'a>(&self, input: &'a str) -> impl Iterator<Item = SegmentData<'a>> {
        input
            .grapheme_indices(true)
            .map(|(index, segment)| SegmentData {
                segment,
                index,
                input,
            })
    }
}
/// Return the shared grapheme segmenter.
#[must_use]
pub fn get_segmenter() -> &'static Segmenter {
    static SEGMENTER: Segmenter = Segmenter;
    &SEGMENTER
}
/// Extract a supported escape at a byte offset, returning code and byte length.
#[must_use]
pub fn extract_ansi_code(text: &str, pos: usize) -> Option<(String, usize)> {
    let rest = text.get(pos..)?;
    let length = ansi_len(rest)?;
    Some((rest[..length].into(), length))
}
/// Whether any supplied character is whitespace.
#[must_use]
pub fn is_whitespace_char(text: &str) -> bool {
    text.chars().any(char::is_whitespace)
}
/// Whether any supplied character belongs to the punctuation set.
#[must_use]
pub fn is_punctuation_char(text: &str) -> bool {
    text.chars()
        .any(|ch| "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`".contains(ch))
}
/// Pad without clipping, then invoke the supplied background style exactly once.
#[must_use]
pub fn apply_background_to_line(
    line: &str,
    width: usize,
    bg_fn: &dyn Fn(&str) -> String,
) -> String {
    bg_fn(&(line.to_owned() + &" ".repeat(width.saturating_sub(visible_width(line)))))
}

mod ansi;
mod columns;
mod truncation;
mod wrapping;
pub use columns::{extract_segments, slice_by_column, slice_with_width};
pub use truncation::truncate_to_width;
pub use wrapping::wrap_text_with_ansi;
