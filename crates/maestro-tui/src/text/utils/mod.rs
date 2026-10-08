#![doc = include_str!("../../../../../docs/terminal/text.md")]

mod columns;
mod measure;
mod parsed;
mod render;
mod style;
mod truncation;
mod wrapping;

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

pub use columns::{
    ColumnSlice, ExtractedSegments, extract_segments, slice_by_column, slice_with_width,
};
pub use measure::visible_width;
pub use truncation::{TruncateOptions, truncate_to_width};
pub use wrapping::wrap_text_with_ansi;

/// A supported escape sequence found at a byte position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnsiCode<'a> {
    /// The escape exactly as written.
    pub code: &'a str,
    /// Length of the escape in UTF-8 bytes.
    pub length: usize,
}

/// Grapheme clusters of `text` with their UTF-8 byte positions.
pub fn get_segmenter(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.grapheme_indices(true)
}

/// Rewrites the Thai and Lao AM vowels as the pairs terminals repaint reliably.
#[must_use]
pub fn normalize_terminal_output(text: &str) -> Cow<'_, str> {
    if !text.contains(['\u{e33}', '\u{eb3}']) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .fold(String::with_capacity(text.len() + 4), |mut out, scalar| {
                match scalar {
                    '\u{e33}' => out.push_str("\u{e4d}\u{e32}"),
                    '\u{eb3}' => out.push_str("\u{ecd}\u{eb2}"),
                    _ => out.push(scalar),
                }
                out
            }),
    )
}

/// The supported escape sequence starting at byte `pos`, if one starts there.
#[must_use]
pub fn extract_ansi_code(text: &str, pos: usize) -> Option<AnsiCode<'_>> {
    parsed::recognize(text, pos).map(|code| AnsiCode {
        code,
        length: code.len(),
    })
}

/// Whether any scalar of `text` is whitespace.
#[must_use]
pub fn is_whitespace_char(text: &str) -> bool {
    text.chars().any(is_whitespace_scalar)
}

/// Whether any scalar of `text` is one of the supported punctuation marks.
#[must_use]
pub fn is_punctuation_char(text: &str) -> bool {
    text.chars()
        .any(|scalar| "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`".contains(scalar))
}

/// Whether one scalar is whitespace in the sense of ECMAScript regular expressions.
pub(crate) const fn is_whitespace_scalar(scalar: char) -> bool {
    matches!(
        scalar,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// Pads `line` with spaces to `width` columns and passes it through `bg_fn` once.
pub fn apply_background_to_line(
    line: &str,
    width: usize,
    bg_fn: impl FnOnce(&str) -> String,
) -> String {
    let padding = width.saturating_sub(visible_width(line));
    bg_fn(&format!("{line}{}", " ".repeat(padding)))
}
