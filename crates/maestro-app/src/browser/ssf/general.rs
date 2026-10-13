//! General scalar and numeric formatting.

use super::super::strip_decimal;

/// Borrowed scalar input to General formatting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeneralValue<'a> {
    /// Text retained without normalization.
    Text(&'a str),
    /// A boolean rendered as an uppercase word.
    Boolean(bool),
    /// A numeric cell value.
    Number(f64),
    /// A missing or null cell value.
    Empty,
}

/// Render a scalar with General formatting.
#[must_use]
pub fn general(value: GeneralValue<'_>) -> String {
    match value {
        GeneralValue::Text(text) => text.to_owned(),
        GeneralValue::Boolean(true) => "TRUE".to_owned(),
        GeneralValue::Boolean(false) => "FALSE".to_owned(),
        GeneralValue::Empty => String::new(),
        GeneralValue::Number(value) => general_number(value),
    }
}

/// Round an exact binary64 expansion to significant decimal digits, ties away from zero.
fn rounded(value: f64, precision: usize) -> (String, i32) {
    // Binary64's smallest exponent is -1074; this expansion includes every decimal digit.
    let exact = format!("{:.1074e}", value.abs());
    let (mantissa, exp) = exact.split_once('e').unwrap_or((&exact, "0"));
    let mut exponent = exp
        .bytes()
        .filter(u8::is_ascii_digit)
        .fold(0_i32, |n, digit| n * 10 + i32::from(digit - b'0'));
    if exp.starts_with('-') {
        exponent = -exponent;
    }
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let carry = digits[precision] >= b'5';
    digits.truncate(precision);
    if carry {
        let mut overflow = true;
        for digit in digits.iter_mut().rev() {
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit += 1;
                overflow = false;
                break;
            }
        }
        if overflow {
            digits[0] = b'1';
            exponent += 1;
        }
    }
    (digits.into_iter().map(char::from).collect(), exponent)
}

/// Render significant digits, optionally forcing scientific notation.
pub(super) fn precision(value: f64, count: usize, exponential: bool) -> String {
    let (digits, exponent) = rounded(value, count);
    let sign = if value < 0.0 { "-" } else { "" };
    let text =
        if exponential || exponent < -6 || exponent >= i32::try_from(count).unwrap_or(i32::MAX) {
            format!("{}.{}e{exponent:+}", &digits[..1], &digits[1..])
        } else if exponent < 0 {
            format!(
                "0.{}{digits}",
                "0".repeat(usize::try_from(-exponent - 1).unwrap_or_default())
            )
        } else {
            let split = usize::try_from(exponent + 1).unwrap_or_default();
            if split == count {
                digits
            } else {
                format!("{}.{}", &digits[..split], &digits[split..])
            }
        };
    format!("{sign}{text}")
}

/// Normalize the exponent and strip only insignificant mantissa zeros.
fn normalize(text: &str) -> String {
    if let Some((mantissa, exponent)) = text.split_once('e') {
        let (sign, digits) = exponent.split_at(1);
        format!("{}E{sign}{digits:0>2}", strip_decimal(mantissa))
    } else {
        strip_decimal(text).to_owned()
    }
}

/// Select numeric spelling using the General width and exponent tiers.
fn general_number(value: f64) -> String {
    if !value.is_finite() {
        return if value.is_nan() { "#NUM!" } else { "#DIV/0!" }.to_owned();
    }
    if value.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value) {
        return ryu_js::Buffer::new().format(value).to_owned();
    }
    let exponent = (value.abs().ln() * std::f64::consts::LOG10_E).floor();
    let width = if value < 0.0 { 12 } else { 11 };
    let text = if (-4.0..=-1.0).contains(&exponent) {
        precision(
            value,
            num_traits::ToPrimitive::to_usize(&(10.0 + exponent)).unwrap_or(6),
            false,
        )
    } else if exponent.abs() <= 9.0 {
        small_number(value, width)
    } else if (10.0..11.0).contains(&exponent) {
        ryu_js::Buffer::new()
            .format_to_fixed(value, 10)
            .chars()
            .take(12)
            .collect()
    } else {
        let mut buffer = ryu_js::Buffer::new();
        let fixed = strip_decimal(buffer.format_to_fixed(value, 11));
        if fixed.len() > width || matches!(fixed, "0" | "-0") {
            precision(value, 6, false)
        } else {
            fixed.to_owned()
        }
    };
    normalize(&text)
}

/// Try fixed and significant spelling before a five-place exponential fallback.
fn small_number(value: f64, width: usize) -> String {
    let mut buffer = ryu_js::Buffer::new();
    let fixed = strip_decimal(buffer.format_to_fixed(value, 12));
    if fixed.len() <= width {
        return fixed.to_owned();
    }
    let significant = precision(value, 10, false);
    if significant.len() <= width {
        significant
    } else {
        precision(value, 6, true)
    }
}
