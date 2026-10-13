//! Decimal text cleanup shared by browser formatting policies.

/// Remove trailing fractional zeros and a trailing decimal point from non-exponential text.
///
/// Integer text and text containing `e` or `E` are returned unchanged.
#[must_use]
pub fn strip_decimal(text: &str) -> &str {
    if text.contains('.') && !text.contains(['e', 'E']) {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}
