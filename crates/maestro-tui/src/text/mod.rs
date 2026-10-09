//! Styled-text measurement and layout.

pub mod utils;

use std::borrow::Cow;

pub(crate) use utils::Endings;

/// Expands displayed tabs to three spaces and maps a byte cursor, preserving recognized escapes.
pub(crate) fn expand_tabs(text: &str, cursor: usize) -> (Cow<'_, str>, usize) {
    if !text.contains('\t') {
        return (Cow::Borrowed(text), cursor);
    }
    let endings = Endings::of(text);
    let mut expanded = String::with_capacity(text.len());
    let mut position = 0;
    let mut mapped = cursor;
    while position < text.len() {
        if let Some(escape) = endings.recognize(text, position) {
            expanded.push_str(escape);
            position += escape.len();
        } else if let Some(scalar) = text[position..].chars().next() {
            if scalar == '\t' {
                expanded.push_str("   ");
                mapped += 2 * usize::from(position < cursor);
            } else {
                expanded.push(scalar);
            }
            position += scalar.len_utf8();
        }
    }
    (Cow::Owned(expanded), mapped)
}
