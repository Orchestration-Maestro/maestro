//! Horizontal viewport and display-only cursor decoration.

use super::InputState;
use crate::{CURSOR_MARKER, extract_ansi_code, get_segmenter, slice_by_column, visible_width};
use std::borrow::Cow;

/// Composes one prompt, horizontally selected text and inverse cursor.
pub(super) fn line(state: &InputState, width: usize, focused: bool) -> String {
    if width <= 2 {
        return "> "[..width].to_owned();
    }
    let available = width - 2;
    let (visible, cursor) = viewport(state, available);
    let cursor = display_cursor(&visible, cursor);
    let at = get_segmenter(&visible[cursor..])
        .next()
        .map_or(" ", |(_, text)| text);
    let after = (cursor + at.len()).min(visible.len());
    let marker = if focused { CURSOR_MARKER } else { "" };
    let decorated = format!(
        "{}{marker}\x1b[7m{at}\x1b[27m{}",
        &visible[..cursor],
        &visible[after..]
    );
    let padding = " ".repeat(available.saturating_sub(visible_width(&decorated)));
    format!("> {decorated}{padding}")
}

/// Selects a strict horizontal window and its local cursor byte offset.
fn viewport(state: &InputState, available: usize) -> (Cow<'_, str>, usize) {
    let cursor = display_cursor(&state.value, state.cursor);
    let total = visible_width(&state.value);
    if total < available {
        return (Cow::Borrowed(&state.value), cursor);
    }
    let scroll = available - usize::from(cursor == state.value.len());
    if scroll == 0 {
        return (Cow::Borrowed(""), 0);
    }
    let col = visible_width(&state.value[..cursor]);
    let half = scroll / 2;
    let start = if col < half {
        0
    } else if col > total.saturating_sub(half) {
        total.saturating_sub(scroll)
    } else {
        col.saturating_sub(half)
    };
    let visible = slice_by_column(&state.value, start, scroll, true);
    let before = slice_by_column(&state.value, start, col.saturating_sub(start), true);
    let cursor = prefix_cursor(&visible, &before);
    (Cow::Owned(visible), cursor)
}

/// Advances a display cursor out of recognized escapes, including adjacent escapes.
fn display_cursor(text: &str, mut cursor: usize) -> usize {
    let mut position = 0;
    while position < text.len() && position <= cursor {
        if let Some(escape) = extract_ansi_code(text, position) {
            let end = position + escape.length;
            if cursor >= position && cursor < end {
                cursor = end;
            }
            position = end;
        } else if let Some(scalar) = text[position..].chars().next() {
            position += scalar.len_utf8();
        }
    }
    cursor
}

/// Counts text bytes without counting recognized terminal escapes.
fn text_bytes(text: &str) -> usize {
    let mut position = 0;
    let mut bytes = 0;
    while position < text.len() {
        if let Some(escape) = extract_ansi_code(text, position) {
            position += escape.length;
        } else if let Some(scalar) = text[position..].chars().next() {
            position += scalar.len_utf8();
            bytes += scalar.len_utf8();
        }
    }
    bytes
}

/// Maps a separately sliced prefix's text bytes into the actual displayed slice.
fn prefix_cursor(text: &str, prefix: &str) -> usize {
    let mut remaining = text_bytes(prefix);
    let mut position = 0;
    while position < text.len() && remaining > 0 {
        if let Some(escape) = extract_ansi_code(text, position) {
            position += escape.length;
        } else if let Some(scalar) = text[position..].chars().next() {
            remaining = remaining.saturating_sub(scalar.len_utf8());
            position += scalar.len_utf8();
        }
    }
    position
}
