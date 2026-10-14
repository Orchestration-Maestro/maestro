//! Lexical comparison in UTF-16 code-unit order.
use std::cmp::Ordering;

/// Compare by UTF-16 code units, so supplementary scalars sort before U+E000.
#[must_use]
pub fn utf16_order(left: &str, right: &str) -> Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

/// Keep the longest prefix of whole characters that fits in `limit` UTF-16 units.
#[must_use]
pub fn truncate_utf16(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|character| {
            units += character.len_utf16();
            units <= limit
        })
        .collect()
}
