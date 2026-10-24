//! Column padding and number spelling shared by the report rows.

use crate::utils::numbers::round;
use maestro_text::number_format::LocaleNumberFormat;

/// The number of UTF-16 code units in `value`, as string padding counts it.
pub(crate) fn units(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Pad `value` on the right to `columns` code units; longer values stay whole.
pub(crate) fn pad_end(value: &str, columns: usize) -> String {
    let mut text = value.to_owned();
    text.extend(std::iter::repeat_n(
        ' ',
        columns.saturating_sub(units(value)),
    ));
    text
}

/// Pad `value` on the left to `columns` code units; longer values stay whole.
pub(crate) fn pad_start(value: &str, columns: usize) -> String {
    let mut text = " ".repeat(columns.saturating_sub(units(value)));
    text.push_str(value);
    text
}

/// Spell `value` with four fractional digits, rounding a tie toward the larger
/// amount.
pub(crate) fn cost(value: f64) -> String {
    ryu_js::Buffer::new().format_to_fixed(value, 4).to_owned()
}

/// Spell a rounded count with en-US digit grouping, keeping a negative zero.
///
/// # Errors
/// Returns the locale formatter construction failure.
pub(crate) fn count(value: f64) -> std::io::Result<String> {
    let format = LocaleNumberFormat::new(&|_| Some("en-US".to_owned()))?;
    format.format(round(value))
}
