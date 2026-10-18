//! Terminal cell widths for individual Unicode scalars.

mod props;

/// Return the terminal cell width of a Unicode scalar: zero, one or two.
///
/// A `char` cannot contain a surrogate or an out-of-range code point.
/// This does not measure grapheme clusters.
///
/// ```
/// use maestro_terminal::unicode::codepoint_width;
///
/// assert_eq!(codepoint_width('a'), 1);
/// assert_eq!(codepoint_width('\u{301}'), 0);
/// assert_eq!(codepoint_width('界'), 2);
/// ```
#[must_use]
pub fn codepoint_width(c: char) -> u8 {
    props::Properties::get(c).width
}
