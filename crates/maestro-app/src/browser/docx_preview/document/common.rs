//! Scalar values used by Word XML properties.
use maestro_text::is_whitespace_scalar;

/// Unit policy for an integer-valued Word property.
#[derive(Clone, Copy)]
pub enum LengthUsage {
    /// Twentieths of a point.
    Dxa,
    /// English metric units.
    Emu,
    /// Half points.
    FontSize,
    /// Eighth points, clamped to border limits.
    Border,
    /// Points.
    Point,
    /// Fiftieths of a percent.
    Percent,
}

/// Convert a length, retaining lowercase px/pt/% when the preceding character is not a line terminator.
#[must_use]
pub fn convert_length(value: Option<&str>, usage: LengthUsage) -> Option<String> {
    let value = value?;
    if ["px", "pt", "%"].iter().any(|unit| {
        value.strip_suffix(unit).is_some_and(|prefix| {
            prefix
                .chars()
                .next_back()
                .is_some_and(|c| !matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
        })
    }) {
        return Some(value.into());
    }
    let multiplier = match usage {
        LengthUsage::Dxa => 0.05,
        LengthUsage::Emu => 1.0 / 12700.0,
        LengthUsage::FontSize => 0.5,
        LengthUsage::Border => 0.125,
        LengthUsage::Point => 1.0,
        LengthUsage::Percent => 0.02,
    };
    let mut number = integer_prefix(value, 10) * multiplier;
    if matches!(usage, LengthUsage::Border) {
        number = number.clamp(0.25, 12.0);
    }
    let unit = if matches!(usage, LengthUsage::Percent) {
        "%"
    } else {
        "pt"
    };
    Some(format!(
        "{}{unit}",
        ryu_js::Buffer::new().format_to_fixed(number, 2)
    ))
}

/// Read exact boolean tokens, otherwise returning the supplied fallback.
#[must_use]
pub fn convert_boolean(value: Option<&str>, default_value: Option<bool>) -> Option<bool> {
    match value {
        Some("1" | "on" | "true") => Some(true),
        Some("0" | "off" | "false") => Some(false),
        _ => default_value,
    }
}

/// Read a signed integer prefix with optional hexadecimal notation.
pub(crate) fn integer_prefix(value: &str, radix: u32) -> f64 {
    let value = value.trim_start_matches(is_whitespace_scalar);
    let (sign, value) = if let Some(rest) = value.strip_prefix('-') {
        (-1.0, rest)
    } else {
        (1.0, value.strip_prefix('+').unwrap_or(value))
    };
    let (radix, value) = if let Some(rest) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        (16, rest)
    } else {
        (radix, value)
    };
    let mut digits = value.chars().map_while(|c| c.to_digit(radix)).peekable();
    if digits.peek().is_none() {
        return f64::NAN;
    }
    sign * digits.fold(0.0, |total, digit| {
        total * f64::from(radix) + f64::from(digit)
    })
}

/// Read a decimal floating-point prefix, including an exponent when complete.
pub(crate) fn float_prefix(value: &str) -> f64 {
    let value = value.trim_start_matches(is_whitespace_scalar);
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    if unsigned.starts_with("Infinity") {
        return if value.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let integer = unsigned.trim_start_matches(|c: char| c.is_ascii_digit());
    let fraction = integer.strip_prefix('.').unwrap_or(integer);
    let remainder = fraction.trim_start_matches(|c: char| c.is_ascii_digit());
    if unsigned.len() - integer.len() + fraction.len() - remainder.len() == 0 {
        return f64::NAN;
    }
    let mut end = value.len() - remainder.len();
    if let Some(exponent) = remainder.strip_prefix(['e', 'E']) {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        let rest = digits.trim_start_matches(|c: char| c.is_ascii_digit());
        if rest.len() < digits.len() {
            end = value.len() - rest.len();
        }
    }
    value[..end].parse().unwrap_or(f64::NAN)
}
