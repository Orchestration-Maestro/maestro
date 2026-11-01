//! Integer prefixes and rounding shared by documents, spreadsheets and reports.

use maestro_text::is_whitespace_scalar;

/// Read a base-ten integer prefix, as an explicit decimal argument does.
#[cfg(not(target_arch = "wasm32"))]
#[must_use]
pub(crate) fn decimal_integer_prefix(value: &str) -> f64 {
    integer(value, 10, false)
}

/// Read a signed integer prefix with optional hexadecimal notation.
#[must_use]
pub(crate) fn integer_prefix(value: &str, radix: u32) -> f64 {
    integer(value, radix, true)
}

/// Read a signed integer prefix of `radix`, admitting hexadecimal notation only when
/// `hexadecimal` does.
fn integer(value: &str, radix: u32, hexadecimal: bool) -> f64 {
    let value = value.trim_start_matches(is_whitespace_scalar);
    let (sign, value) = if let Some(rest) = value.strip_prefix('-') {
        (-1.0, rest)
    } else {
        (1.0, value.strip_prefix('+').unwrap_or(value))
    };
    let (radix, value) = match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some(rest) if hexadecimal => (16, rest),
        _ => (radix, value),
    };
    let mut digits = value.chars().map_while(|c| c.to_digit(radix)).peekable();
    if digits.peek().is_none() {
        return f64::NAN;
    }
    sign * digits.fold(0.0, |total, digit| {
        total * f64::from(radix) + f64::from(digit)
    })
}

/// Round to the nearest integer, moving an exact half toward positive infinity
/// and keeping the sign of a rounded negative zero.
#[must_use]
pub(crate) fn round(value: f64) -> f64 {
    let floor = value.floor();
    let rounded = if value - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    };
    if rounded == 0.0 && value.is_sign_negative() {
        -0.0
    } else {
        rounded
    }
}
