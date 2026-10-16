//! Spreadsheet scalar formatting primitives.

use std::collections::BTreeMap;

mod datenum;
mod read;
mod styles;
mod unescapexml;
pub use read::read;
mod address;
mod workbook;
pub use address::{
    CellAddress, Range, decode_cell, decode_range, encode_cell, encode_col, encode_range,
    encode_row,
};
pub use workbook::{WorkBook, WorkSheet, WorkbookError};
mod format;
mod format_cell;
pub use format::{FormatOptions, FormatTable, NumberFormat, Ssf};
pub use format_cell::{CellFormatOptions, CellObject, CellValue, ExcelDataType, Xf, format_cell};
mod chkcond;
mod eval_fmt;
mod general;
mod is_date;
pub use is_date::is_date;
mod parse_date_code;
mod split_fmt;
mod write_date;
mod write_num;
pub use general::{GeneralValue, general};
pub use parse_date_code::{DateCode, parse_date_code};
pub use write_date::write_date;
pub use write_num::write_num;

/// Initial built-in format strings.
const BUILTINS: &[(u32, &str)] = &[
    (0, "General"),
    (1, "0"),
    (2, "0.00"),
    (3, "#,##0"),
    (4, "#,##0.00"),
    (9, "0%"),
    (10, "0.00%"),
    (11, "0.00E+00"),
    (12, "# ?/?"),
    (13, "# ??/??"),
    (14, "m/d/yy"),
    (15, "d-mmm-yy"),
    (16, "d-mmm"),
    (17, "mmm-yy"),
    (18, "h:mm AM/PM"),
    (19, "h:mm:ss AM/PM"),
    (20, "h:mm"),
    (21, "h:mm:ss"),
    (22, "m/d/yy h:mm"),
    (37, "#,##0 ;(#,##0)"),
    (38, "#,##0 ;[Red](#,##0)"),
    (39, "#,##0.00;(#,##0.00)"),
    (40, "#,##0.00;[Red](#,##0.00)"),
    (45, "mm:ss"),
    (46, "[h]:mm:ss"),
    (47, "mmss.0"),
    (48, "##0.0E+0"),
    (49, "@"),
    (56, "\"上午/下午 \"hh\"時\"mm\"分\"ss\"秒 \""),
];

/// Overwrite built-in IDs, retaining other entries in the supplied table.
pub fn init_table(table: &mut BTreeMap<u32, String>) -> &mut BTreeMap<u32, String> {
    table.extend(BUILTINS.iter().map(|&(id, text)| (id, text.to_owned())));
    table
}

/// Look up a defined fallback format ID, independently of built-in entries.
#[must_use]
pub fn default_map(index: u32) -> Option<u32> {
    Some(match index {
        5..=8 => index + 32,
        23..=26 | 82 => 0,
        27..=31 | 50..=58 | 71..=72 => 14,
        59..=62 | 67..=68 | 73..=75 => index - 58,
        69..=70 => index - 57,
        76..=78 => index - 56,
        79..=81 => index - 34,
        _ => return None,
    })
}

/// Look up a defined currency or accounting pattern.
#[must_use]
pub fn default_str(index: u32) -> Option<&'static str> {
    Some(match index {
        5 | 63 => r##""$"#,##0_);\("$"#,##0\)"##,
        6 | 64 => r##""$"#,##0_);[Red]\("$"#,##0\)"##,
        7 | 65 => r##""$"#,##0.00_);\("$"#,##0.00\)"##,
        8 | 66 => r##""$"#,##0.00_);[Red]\("$"#,##0.00\)"##,
        41 => r#"_(* #,##0_);_(* \(#,##0\);_(* "-"_);_(@_)"#,
        42 => r#"_("$"* #,##0_);_("$"* \(#,##0\);_("$"* "-"_);_(@_)"#,
        43 => r#"_(* #,##0.00_);_(* \(#,##0.00\);_(* "-"??_);_(@_)"#,
        44 => r#"_("$"* #,##0.00_);_("$"* \(#,##0.00\);_("$"* "-"??_);_(@_)"#,
        _ => return None,
    })
}

/// Approximate a value with bounded convergents as `[whole, numerator, denominator]`.
///
/// Format writers supply positive denominator limits. Mixed negative values use floor division.
#[must_use]
pub fn frac(value: f64, denominator: u32, mixed: bool) -> [f64; 3] {
    let sign = if value < 0.0 { -1.0 } else { 1.0 };
    let limit = f64::from(denominator);
    let mut residual = value * sign;
    let (mut older, mut previous, mut candidate) = ([0.0, 1.0], [1.0, 0.0], [0.0, 0.0]);
    while previous[1] < limit {
        let integral = residual.floor();
        candidate = [
            integral * previous[0] + older[0],
            integral * previous[1] + older[1],
        ];
        if residual - integral < 0.000_000_05 {
            break;
        }
        residual = 1.0 / (residual - integral);
        older = previous;
        previous = candidate;
    }
    if candidate[1] > limit {
        candidate = if previous[1] > limit { older } else { previous };
    }
    let [numerator, divisor] = candidate;
    let signed = sign * numerator;
    if mixed {
        let whole = (signed / divisor).floor();
        [whole, signed - whole * divisor, divisor]
    } else {
        [0.0, signed, divisor]
    }
}

/// Selected numeric writer mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumberToken {
    /// Ordinary numeric placeholders.
    Number,
    /// Accounting parentheses around negative values.
    Parenthesis,
}

/// Selected calendar or time field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DateToken {
    /// Calendar year with the Buddhist offset.
    BuddhistYear,
    /// Gregorian calendar year.
    Year,
    /// Calendar month or its English name.
    Month,
    /// Calendar day or weekday name.
    Day,
    /// Twelve-hour clock hour.
    Hour12,
    /// Twenty-four-hour clock hour.
    Hour24,
    /// Clock minute.
    Minute,
    /// Clock second or fractional second.
    Second,
    /// Total elapsed hours, minutes or seconds.
    Elapsed,
    /// Untruncated calendar year.
    Era,
}

/// Failure to render a selected field pattern.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatError {
    /// A format contains more than four sections.
    TooManySections,
    /// The selected date field does not support this pattern.
    InvalidDateFormat(DateToken),
    /// The numeric pattern is unsupported.
    UnsupportedNumberFormat,
    /// A quoted literal has no closing quote.
    UnterminatedString,
    /// A bracketed format block has no closing bracket.
    UnterminatedBlock,
    /// A scalar is not part of the admitted format grammar.
    UnrecognizedCharacter(char),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManySections => formatter.write_str("too many format sections"),
            Self::InvalidDateFormat(kind) => write!(formatter, "invalid date format for {kind:?}"),
            Self::UnsupportedNumberFormat => formatter.write_str("unsupported number format"),
            Self::UnterminatedString => formatter.write_str("unterminated string"),
            Self::UnterminatedBlock => formatter.write_str("unterminated bracket block"),
            Self::UnrecognizedCharacter(c) => write!(formatter, "unrecognized character {c}"),
        }
    }
}

impl std::error::Error for FormatError {}

#[cfg(test)]
mod tests;
