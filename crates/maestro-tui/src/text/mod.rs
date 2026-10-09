//! Styled-text measurement and layout.

pub mod utils;

use std::borrow::Cow;

pub(crate) use utils::Endings;

/// Expands displayed tabs to three spaces, preserving recognized escapes.
pub(crate) fn expand_tabs(text: &str) -> Cow<'_, str> {
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }
    let endings = Endings::of(text);
    let mut expanded = String::with_capacity(text.len());
    let mut position = 0;
    while position < text.len() {
        if let Some(escape) = endings.recognize(text, position) {
            expanded.push_str(escape);
            position += escape.len();
        } else if let Some(scalar) = text[position..].chars().next() {
            if scalar == '\t' {
                expanded.push_str("   ");
            } else {
                expanded.push(scalar);
            }
            position += scalar.len_utf8();
        }
    }
    Cow::Owned(expanded)
}
