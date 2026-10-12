//! Lexical comparison in UTF-16 code-unit order.
use std::cmp::Ordering;

/// Compare by UTF-16 code units, so supplementary scalars sort before U+E000.
#[must_use]
pub fn utf16_order(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}
