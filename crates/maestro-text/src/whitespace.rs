//! The whitespace scalar set shared by Maestro text handling.

/// Every whitespace scalar: Unicode white space except U+0085, plus U+FEFF.
pub const WHITESPACE: &str = "\u{9}\u{a}\u{b}\u{c}\u{d}\u{20}\u{a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";

/// Whether `value` belongs to [`WHITESPACE`].
#[must_use]
pub fn is_whitespace_scalar(value: char) -> bool {
    WHITESPACE.contains(value)
}
