//! Horizontal viewport and display-only cursor decoration.

use super::InputState;
use crate::components::cursor::display_cursor;
use crate::{CURSOR_MARKER, get_segmenter, slice_by_column, visible_width};
use std::borrow::Cow;

/// Composes one prompt, horizontally selected text and inverse cursor.
pub(super) fn line(state: &InputState, width: usize, focused: bool) -> String {
    if width <= 2 {
        return "> "[..width].to_owned();
    }
    let available = width - 2;
    let cursor = display_cursor(&state.value, state.cursor);
    let text = crate::text::expand_tabs(&state.value);
    let cursor = crate::text::expand_tabs(&state.value[..cursor]).len();
    let (visible, cursor) = viewport(&text, cursor, available);
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

/// Selects text by columns, reserving a blank when its mapped cursor reaches the end.
fn viewport(text: &str, cursor: usize, available: usize) -> (Cow<'_, str>, usize) {
    let cursor = display_cursor(text, cursor);
    let total = visible_width(text);
    if total < available {
        return (Cow::Borrowed(text), cursor);
    }
    let scroll = available - usize::from(cursor == text.len());
    let (visible, mapped) = select(text, cursor, total, scroll);
    let mapped = display_cursor(&visible, mapped);
    if mapped == visible.len() && visible_width(&visible) >= available {
        let start = total.saturating_sub(available - 1);
        let visible = slice_by_column(text, start, available - 1, true);
        let before = slice_by_column(
            text,
            start,
            visible_width(&text[..cursor]).saturating_sub(start),
            true,
        );
        let mapped = prefix_cursor(&visible, &before);
        (Cow::Owned(visible), mapped)
    } else {
        (visible, mapped)
    }
}

/// Selects text around the display cursor with the requested cell budget.
fn select(text: &str, cursor: usize, total: usize, scroll: usize) -> (Cow<'_, str>, usize) {
    if scroll == 0 {
        return (Cow::Borrowed(""), 0);
    }
    let col = visible_width(&text[..cursor]);
    let half = scroll / 2;
    let start = if col < half {
        0
    } else if col > total.saturating_sub(half) {
        total.saturating_sub(scroll)
    } else {
        col.saturating_sub(half)
    };
    let visible = slice_by_column(text, start, scroll, true);
    let before = slice_by_column(text, start, col.saturating_sub(start), true);
    let cursor = prefix_cursor(&visible, &before);
    (Cow::Owned(visible), cursor)
}

/// Counts text bytes without counting recognized terminal escapes.
fn text_bytes(text: &str) -> usize {
    let endings = crate::text::Endings::of(text);
    let mut position = 0;
    let mut bytes = 0;
    while position < text.len() {
        if let Some(escape) = endings.recognize(text, position) {
            position += escape.len();
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
    let endings = crate::text::Endings::of(text);
    let mut position = 0;
    while position < text.len() && remaining > 0 {
        if let Some(escape) = endings.recognize(text, position) {
            position += escape.len();
        } else if let Some(scalar) = text[position..].chars().next() {
            remaining = remaining.saturating_sub(scalar.len_utf8());
            position += scalar.len_utf8();
        }
    }
    position
}
