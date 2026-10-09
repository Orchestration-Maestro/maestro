//! Styled-text measurement and layout.

pub mod utils;

use std::borrow::Cow;

pub(crate) use utils::Endings;

/// Replaces each literal tab with the three spaces used by terminal width measurement.
pub(crate) fn expand_tabs(text: &str) -> Cow<'_, str> {
    if text.contains('\t') {
        Cow::Owned(text.replace('\t', "   "))
    } else {
        Cow::Borrowed(text)
    }
}
