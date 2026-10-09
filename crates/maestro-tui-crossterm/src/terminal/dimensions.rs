//! Terminal dimensions: the size of standard output, then the environment, then a default.

use std::cell::Cell;

use rustix::stdio::stdout;
use rustix::termios::tcgetwinsize;

/// The width used when neither the terminal nor `COLUMNS` supplies a positive one.
const DEFAULT_COLUMNS: usize = 80;

/// The height used when neither the terminal nor `LINES` supplies a positive one.
const DEFAULT_ROWS: usize = 24;

/// A window size, where zero means the terminal reported no extent on that axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Size {
    /// Width in columns.
    columns: usize,
    /// Height in rows.
    rows: usize,
}

impl Size {
    /// The size of standard output, or `None` when standard output reports none.
    fn query() -> Option<Self> {
        let size = tcgetwinsize(stdout()).ok()?;
        Some(Self {
            columns: usize::from(size.ws_col),
            rows: usize::from(size.ws_row),
        })
    }
}

/// The size of standard output as last observed.
#[derive(Debug, Default)]
pub(super) struct Dimensions(Cell<Size>);

impl Dimensions {
    /// Observes the size of standard output now.
    pub(super) fn observe() -> Self {
        let dimensions = Self::default();
        dimensions.refresh();
        dimensions
    }

    /// Observes the size again and reports whether it differs from the last observation.
    pub(super) fn refresh(&self) -> bool {
        self.apply(Size::query())
    }

    /// Records `observed` and reports whether it differs. An absent observation changes nothing.
    fn apply(&self, observed: Option<Size>) -> bool {
        observed.is_some_and(|size| self.0.replace(size) != size)
    }

    /// The observed width when positive, else a valid `COLUMNS`, else 80.
    pub(super) fn columns(&self) -> usize {
        extent(self.0.get().columns, "COLUMNS", DEFAULT_COLUMNS)
    }

    /// The observed height when positive, else a valid `LINES`, else 24.
    pub(super) fn rows(&self) -> usize {
        extent(self.0.get().rows, "LINES", DEFAULT_ROWS)
    }
}

/// `observed` when positive, else the extent `variable` holds, else `default`.
fn extent(observed: usize, variable: &str, default: usize) -> usize {
    if observed > 0 {
        return observed;
    }
    std::env::var(variable)
        .ok()
        .and_then(|text| numeric_extent(&text))
        .unwrap_or(default)
}

/// Whether numeric text may be surrounded by `character`: Unicode white space, line
/// terminators included, except U+0085, and U+FEFF.
fn is_numeric_whitespace(character: char) -> bool {
    character == '\u{feff}' || (character != '\u{85}' && character.is_whitespace())
}

/// The extent `text` reads as when it is a number that is whole and from 1 to `usize::MAX`;
/// empty, zero, negative, fractional, nonnumeric, non-finite and larger values give `None`.
fn numeric_extent(text: &str) -> Option<usize> {
    let value = numeric_value(text.trim_matches(is_numeric_whitespace))?;
    (value >= 1.0 && value.fract() == 0.0)
        .then(|| format!("{value:.0}").parse().ok())
        .flatten()
}

/// The prefixes that select an integer in another base, case-insensitively.
const RADIX_PREFIXES: [(&str, u32); 3] = [("0x", 16), ("0b", 2), ("0o", 8)];

/// The number `text` reads as, without surrounding whitespace, or `None` when it is not a
/// number or is an integer in another base too large for any extent. Empty text is zero.
/// Decimal text takes an optional sign, a fraction and an exponent.
fn numeric_value(text: &str) -> Option<f64> {
    if text.is_empty() {
        return Some(0.0);
    }
    if let Some(head) = text.get(..2) {
        for (prefix, radix) in RADIX_PREFIXES {
            if head.eq_ignore_ascii_case(prefix) {
                return radix_value(text.get(2..)?, radix);
            }
        }
    }
    text.parse().ok()
}

/// The value of `digits` in `radix`, rounded to the nearest `f64`, when every character is a
/// digit of that base; no sign is allowed.
fn radix_value(digits: &str, radix: u32) -> Option<f64> {
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return None;
    }
    u128::from_str_radix(digits, radix)
        .ok()?
        .to_string()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::{Dimensions, Size, numeric_extent};

    /// The size observed last.
    fn observed(dimensions: &Dimensions) -> Size {
        dimensions.0.get()
    }

    #[test]
    fn a_changed_size_is_reported_once_and_an_unchanged_one_is_not() {
        let dimensions = Dimensions::default();
        let (small, large) = (
            Size {
                columns: 80,
                rows: 24,
            },
            Size {
                columns: 100,
                rows: 24,
            },
        );
        assert!(dimensions.apply(Some(small)));
        assert!(!dimensions.apply(Some(small)));
        assert!(dimensions.apply(Some(large)));
        assert_eq!(observed(&dimensions), large);
    }

    #[test]
    fn a_failed_query_keeps_the_last_observed_size_and_reports_no_change() {
        let dimensions = Dimensions::default();
        let size = Size {
            columns: 80,
            rows: 24,
        };
        dimensions.apply(Some(size));
        assert!(!dimensions.apply(None));
        assert_eq!(observed(&dimensions), size);
    }

    #[test]
    fn a_size_of_zero_replaces_the_observation_and_is_a_change() {
        let dimensions = Dimensions::default();
        dimensions.apply(Some(Size {
            columns: 80,
            rows: 24,
        }));
        assert!(dimensions.apply(Some(Size::default())));
        assert_eq!(observed(&dimensions), Size::default());
    }

    #[test]
    fn numeric_text_gives_whole_number_extents() {
        let accepted: [(&str, usize); 27] = [
            ("123", 123),
            (" 123 ", 123),
            ("\u{9}123\u{d}\u{a}", 123),
            ("00080", 80),
            ("+42", 42),
            ("1e2", 100),
            ("1E2", 100),
            ("1e+2", 100),
            ("1.0", 1),
            ("42.", 42),
            ("42.0", 42),
            (".5e1", 5),
            ("0x50", 80),
            ("0X50", 80),
            ("0b1010000", 80),
            ("0B101", 5),
            ("0o120", 80),
            ("0O12", 10),
            ("9007199254740993", 9_007_199_254_740_992),
            ("9007199254740992", 9_007_199_254_740_992),
            ("18446744073709550591", 18_446_744_073_709_549_568),
            ("4294967296", 4_294_967_296),
            ("0x7ffffffffffffc00", 9_223_372_036_854_774_784),
            ("0x20000000000001", 9_007_199_254_740_992),
            ("1 ", 1),
            ("1.e1", 10),
            ("0.0000001e7", 1),
        ];
        for (text, expected) in accepted {
            assert_eq!(numeric_extent(text), Some(expected), "{text:?}");
        }
    }

    #[test]
    fn text_without_a_usable_numeric_value_gives_no_extent() {
        let rejected: [&str; 48] = [
            "-42",
            "+0",
            "-0",
            "0",
            "",
            "   ",
            "1e-2",
            "1.5",
            ".5",
            "5e-1",
            "0x",
            "0x+1",
            "-0x10",
            "+0x10",
            "0xg",
            "0b2",
            "0o8",
            "0x 10",
            "1_000",
            "Infinity",
            "-Infinity",
            "+Infinity",
            "infinity",
            "NaN",
            "nan",
            "inf",
            "1e400",
            "1e308",
            "18446744073709551615",
            "18446744073709551616",
            "18446744073709553664",
            "0x10000000000000000",
            "0xffffffffffffffff",
            "12 3",
            "\u{0}",
            "123\u{0}",
            "1e",
            "e1",
            ".",
            "+.5",
            "00",
            "1e1000",
            "\u{ff11}\u{ff12}\u{ff13}",
            "\u{661}\u{662}\u{663}",
            "\u{85}123\u{85}",
            "\u{180e}123\u{180e}",
            "\u{200b}123",
            "\u{2060}123",
        ];
        for text in rejected {
            assert_eq!(numeric_extent(text), None, "{text:?}");
        }
    }

    #[test]
    fn exactly_unicode_spaces_and_feff_are_skipped_around_numbers() {
        let skipped = [
            0x9, 0xa, 0xb, 0xc, 0xd, 0x20, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004,
            0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
            0xfeff,
        ];
        let kept = [
            0x0, 0x1c, 0x1d, 0x1e, 0x1f, 0x85, 0x180e, 0x200b, 0x200c, 0x200d, 0x2060,
        ];
        for code in skipped {
            let space = char::from_u32(code).unwrap();
            assert_eq!(
                numeric_extent(&format!("{space}42{space}")),
                Some(42),
                "{code:x}"
            );
        }
        for code in kept {
            let other = char::from_u32(code).unwrap();
            assert_eq!(
                numeric_extent(&format!("{other}42{other}")),
                None,
                "{code:x}"
            );
        }
    }
}
