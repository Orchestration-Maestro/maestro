//! Numeric pattern writing.

use super::FormatError::UnsupportedNumberFormat as Unsupported;
use super::{FormatError, NumberToken};
use num_traits::ToPrimitive;

pub(super) use crate::utils::numbers::round;

/// Shortest binary64 decimal spelling.
pub(super) fn spelling(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

/// Expand unfilled placeholders.
fn placeholders(format: &str) -> String {
    format.replace('#', "").replace('?', " ")
}

/// Pad a decimal observation with leading zeroes.
pub(super) fn pad(value: f64, width: usize) -> String {
    format!("{:0>width$}", spelling(value))
}

/// Render an already-selected numeric pattern, not a complete cell format.
///
/// # Errors
/// Returns an unsupported-pattern error when no numeric form accepts the field.
pub fn write_num(kind: NumberToken, format: &str, value: f64) -> Result<String, FormatError> {
    let phone = format
        .split(')')
        .skip(1)
        .any(|part| part.contains(['0', '#']));
    if kind == NumberToken::Parenthesis && !phone {
        let field = format
            .replacen('(', "", 1)
            .trim_start_matches(' ')
            .replacen(" )", "", 1)
            .replacen(')', "", 1);
        let text = write_num(NumberToken::Number, &field, value.abs())?;
        return Ok(if value < 0.0 {
            format!("({text})")
        } else {
            text
        });
    }
    let unscaled = format.trim_end_matches(',');
    if unscaled.len() < format.len() {
        return write_num(
            kind,
            unscaled,
            value
                / 10_f64.powi(
                    (3 * (format.len() - unscaled.len()))
                        .to_i32()
                        .unwrap_or(i32::MAX),
                ),
        );
    }
    let percent = format.bytes().filter(|c| *c == b'%').count();
    if percent > 0 {
        return Ok(write_num(
            kind,
            &format.replace('%', ""),
            value * 10_f64.powi((2 * percent).to_i32().unwrap_or(i32::MAX)),
        )? + &"%".repeat(percent));
    }
    if format.contains('E') {
        return scientific(format, value);
    }
    if let Some(field) = format.strip_prefix('$') {
        return Ok(format!(
            "${}",
            write_num(kind, field.strip_prefix(' ').unwrap_or(field), value)?
        ));
    }
    if format.contains('-') || format.contains('(') {
        return mask(kind, format, value);
    }
    plain(format, value)
}

/// Render placeholders after scale and presentation modifiers.
fn plain(format: &str, value: f64) -> Result<String, FormatError> {
    let sign = sign(value);
    if format.contains('/') {
        return fraction(format, value);
    }
    let required = format.trim_start_matches('#');
    if field(required, "0") && format.len() > 1 {
        return Ok(format!("{sign}{}", pad(round(value.abs()), required.len())));
    }
    if field(format, "#0?") {
        let mut text = spelling(round(value));
        if text == "0" && !format.contains('0') {
            text.clear();
        }
        let missing = format.len().saturating_sub(text.len());
        return Ok(format!("{}{text}", placeholders(&format[..missing])));
    }
    if format.contains(',') {
        return grouped(format, value);
    }
    decimal(format, value)
}

/// Select prefix decimals, aligned fields or the integral suffix fallback.
fn decimal(format: &str, value: f64) -> Result<String, FormatError> {
    if let Some((integer, fraction)) = format.split_once('.') {
        let length = fraction
            .bytes()
            .take_while(|c| matches!(c, b'0' | b'#'))
            .count();
        if (integer.trim_start_matches('#').is_empty()
            || field(integer.trim_start_matches('#'), "0"))
            && (length > 0 || fraction.is_empty())
        {
            return Ok(decimal_field(format, &fraction[..length], value));
        }
        if field(integer, "#0?") && field(fraction, "#0") {
            return Ok(aligned_decimal(integer, fraction, value));
        }
    }
    decimal_fallback(format, value)
}

/// Preserve the integral suffix fallback without widening float recognition.
fn decimal_fallback(format: &str, value: f64) -> Result<String, FormatError> {
    if integer_route(value)
        && let Some((integer, fraction)) = format.rsplit_once('.')
        && (fraction.is_empty() || field(fraction, "#0?"))
    {
        return Ok(write_num(NumberToken::Number, integer, value)?
            + &placeholders(&format!(".{fraction}")));
    }
    Err(Unsupported)
}

/// Render required fractional zeroes without globally trimming the decimal point.
fn decimal_field(format: &str, fraction: &str, value: f64) -> String {
    let (sign, magnitude) = if fraction.is_empty() {
        (sign(value), value.abs())
    } else {
        ("", value)
    };
    let text = rounded_decimal(magnitude, fraction.len());
    let (whole, digits) = text.split_once('.').unwrap_or((&text, ""));
    let whole = if whole == "0" && !format.contains("0.") {
        ""
    } else {
        whole
    };
    let missing = placeholders(fraction).len().saturating_sub(digits.len());
    format!("{sign}{whole}.{digits}{}", "0".repeat(missing))
}

/// Align question fields around the capped fixed-decimal observation.
fn aligned_decimal(integer: &str, fraction: &str, value: f64) -> String {
    let text = ryu_js::Buffer::new()
        .format_to_fixed(value, fraction.len().min(10).to_u8().unwrap_or(10))
        .to_owned();
    let text = if text.contains('.') {
        text.trim_end_matches('0')
    } else {
        &text
    };
    let whole = text.find('.').unwrap_or(text.len());
    let left = integer.len().saturating_sub(whole);
    let right = fraction
        .len()
        .saturating_sub(text.len().saturating_sub(whole + 1));
    placeholders(&format!(
        "{}{text}{}",
        &integer[..left],
        &fraction[fraction.len() - right..]
    ))
}

/// Render fractional rounding without scaling an already integral observation.
fn rounded_decimal(value: f64, digits: usize) -> String {
    if value.fract() == 0.0 {
        return format!("{:.0}", if value == 0.0 { 0.0 } else { value });
    }
    let scale = 10_f64.powi(digits.to_i32().unwrap_or(i32::MAX));
    spelling(round(value * scale) / scale)
}

/// Insert fixed groups of three into an ASCII integer observation.
fn commaify(text: &str) -> String {
    text.chars()
        .enumerate()
        .fold(String::new(), |mut out, (index, c)| {
            if index > 0 && (text.len() - index).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
            out
        })
}

/// Render supported grouped aliases with one fractional carry observation.
fn grouped(format: &str, value: f64) -> Result<String, FormatError> {
    let sign = sign(value);
    let (integer, fraction) = format.split_once('.').unwrap_or((format, ""));
    if let Some(tail) = format.strip_prefix("#,") {
        let hashes = tail.bytes().take_while(|c| *c == b'#').count();
        if tail[hashes..].starts_with(",#0") {
            return write_num(NumberToken::Number, &tail[hashes + 1..], value);
        }
    }
    let prefix = integer.strip_suffix(",##0");
    let standard = prefix.is_some_and(|p| (1..=3).contains(&p.len()) && field(p, "#"));
    let optional = matches!(integer, "###,###" | "##,###" | "#,###");
    let padded = integer == "00,000";
    let simple = (standard || optional) && !format.contains('.');
    let point = standard && fraction.is_empty();
    let decimal = (integer == "#,##0"
        || padded
        || matches!(format, "###,##0.00" | "###,###.00" | "#,###.00"))
        && field(fraction, "#0")
        && fraction.ends_with('0');
    if !simple && !point && !decimal {
        return decimal_fallback(format, value);
    }
    let scale = 10_f64.powi(fraction.len().to_i32().unwrap_or(i32::MAX));
    let whole = value.abs().floor();
    let remainder = round((value.abs() - whole) * scale);
    let carry = f64::from(u8::from(remainder >= scale));
    let whole = spelling(whole + carry);
    let digits = pad(if carry == 0.0 { remainder } else { 0.0 }, fraction.len());
    if optional && whole == "0" && !format.contains('.') {
        return Ok(String::new());
    }
    let width = if padded { 5 } else { 0 };
    let tail = if fraction.is_empty() {
        String::new()
    } else {
        format!(".{digits:0<width$}", width = fraction.len())
    };
    let mut out = format!("{sign}{}{tail}", commaify(&format!("{whole:0>width$}")));
    if optional && out.starts_with("0.") {
        out.remove(0);
    }
    Ok(out)
}

/// Whether the source selects its exact signed-integer shortcut.
fn integer_route(value: f64) -> bool {
    value.fract() == 0.0 && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&value)
}

/// Borrowed fraction fields with their literal slash spacing.
struct Fraction<'a> {
    /// Numerator placeholder field.
    numerator: &'a str,
    /// Denominator field.
    denominator: &'a str,
    /// Space before the slash.
    before: &'a str,
    /// Space after the slash.
    after: &'a str,
}

/// Parse fraction fields and dispatch literal versus approximated denominators.
fn fraction(format: &str, value: f64) -> Result<String, FormatError> {
    let (left, right) = format.split_once('/').ok_or(Unsupported)?;
    let after = if right.starts_with(' ') { " " } else { "" };
    let right = right.strip_prefix(' ').unwrap_or(right);
    let literal = left
        .rsplit_once("# ")
        .is_some_and(|(_, n)| field(n.strip_suffix(' ').unwrap_or(n), "?"))
        && right.starts_with(|c: char| c.is_ascii_digit());
    let numerator = if literal {
        left.rsplit_once("# ").map_or(left, |(_, n)| n)
    } else {
        left.strip_prefix("# ").unwrap_or(left)
    };
    let length = right
        .bytes()
        .take_while(|c| {
            if literal {
                c.is_ascii_digit()
            } else {
                matches!(c, b'#' | b'?' | b'0')
            }
        })
        .count();
    let parts = Fraction {
        numerator: numerator.strip_suffix(' ').unwrap_or(numerator),
        denominator: &right[..length],
        before: if numerator.ends_with(' ') { " " } else { "" },
        after,
    };
    if literal {
        return fixed_fraction(&parts, value);
    }
    if !field(parts.numerator, "#0?") || parts.denominator.is_empty() {
        return Err(Unsupported);
    }
    approximate_fraction(&parts, value, format.starts_with("# "))
}

/// Render one bounded approximation with the selected whole-part policy.
fn approximate_fraction(
    field: &Fraction<'_>,
    value: f64,
    mixed: bool,
) -> Result<String, FormatError> {
    let width = if mixed {
        field.numerator.len().max(field.denominator.len())
    } else {
        field.denominator.len()
    }
    .min(7);
    let [whole, numerator, denominator] = super::frac(
        value.abs(),
        10_u32.pow(width.to_u32().unwrap_or(7)) - 1,
        mixed,
    );
    let sign = sign(value);
    if mixed {
        let whole = if whole != 0.0 || numerator == 0.0 {
            spelling(whole)
        } else {
            String::new()
        };
        let fraction = if numerator == 0.0 {
            " ".repeat(2 * width + 1 + field.before.len() + field.after.len())
        } else {
            format!(
                "{:>width$}{}/{}{:<width$}",
                spelling(numerator),
                field.before,
                field.after,
                spelling(denominator)
            )
        };
        return Ok(format!("{sign}{whole} {fraction}"));
    }
    let mut text = write_num(NumberToken::Number, field.numerator, numerator)?;
    if text.ends_with(' ') {
        text.pop();
        text.push('0');
    }
    let denominator = format!("{:<width$}", spelling(denominator));
    let missing = field.denominator.len().saturating_sub(denominator.len());
    let prefix = if missing > 0 {
        placeholders(&field.denominator[missing..])
    } else {
        String::new()
    };
    Ok(format!(
        "{sign}{text}{}/{}{prefix}{denominator}",
        field.before, field.after
    ))
}

/// Render a literal-denominator mixed fraction.
fn fixed_fraction(field: &Fraction<'_>, value: f64) -> Result<String, FormatError> {
    let divisor: f64 = field.denominator.parse().map_err(|_| Unsupported)?;
    if divisor == 0.0 {
        return Err(Unsupported);
    }
    let rounded = round(value.abs() * divisor);
    let (whole, remainder) = if integer_route(value) {
        (value.abs(), 0.0)
    } else {
        let whole = (rounded / divisor).floor();
        (whole, rounded - whole * divisor)
    };
    let whole = if whole == 0.0 {
        String::new()
    } else {
        spelling(whole)
    };
    let width = field.numerator.len();
    let denominator = pad(divisor, field.denominator.len());
    let fraction = if remainder == 0.0 {
        " ".repeat(width + 1 + denominator.len())
    } else {
        format!(
            "{:>width$}{}/{}{denominator}",
            spelling(remainder),
            field.before,
            field.after
        )
    };
    Ok(format!("{}{whole} {fraction}", sign(value)))
}

/// Apply phone slices or right-to-left numeric mask positions.
fn mask(kind: NumberToken, format: &str, value: f64) -> Result<String, FormatError> {
    if format.contains("(###) ###-####") || format.contains(r"(###) ###\-####") {
        let text = write_num(kind, "##########", value)?;
        return Ok(format!(
            "({}) {}-{}",
            &text[..text.len().min(3)],
            &text[text.len().min(3)..text.len().min(6)],
            &text[text.len().min(6)..]
        ));
    }
    let (head, tail) = format.rsplit_once('-').ok_or(Unsupported)?;
    if !head
        .split('-')
        .all(|part| field(part.strip_suffix('\\').unwrap_or(part), "#0"))
        || !field(tail, "#0")
    {
        return Err(Unsupported);
    }
    let format = format.replace('\\', "");
    let text = write_num(kind, &format.replace('-', ""), value)?;
    let mut digits = text.chars().rev();
    let reversed: String = format
        .chars()
        .rev()
        .filter_map(|c| match c {
            '0' | '#' => digits.next().or(if c == '0' { Some('0') } else { None }),
            c => Some(c),
        })
        .collect();
    Ok(reversed.chars().rev().collect())
}

/// Render significant digits and the selected exponent sign/width policy.
fn scientific(format: &str, value: f64) -> Result<String, FormatError> {
    if let Some(prefix) = format.strip_suffix("0.0E+0")
        && field(prefix, "#")
    {
        return Ok(engineering(value, prefix.len() + 1));
    }
    let position = format.find('E').ok_or(Unsupported)?;
    let digits = format
        .find('.')
        .map_or(position, |point| position.saturating_sub(point + 1));
    let text = super::general::precision(value, digits + 1, true);
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "+0"));
    let exponent = if format.ends_with("E+00") && exponent.len() == 2 {
        format!("{}0{}", &exponent[..1], &exponent[1..])
    } else {
        exponent.to_owned()
    };
    let exponent = if format.contains("E-") {
        exponent.strip_prefix('+').unwrap_or(&exponent)
    } else {
        &exponent
    };
    Ok(format!("{mantissa}E{exponent}"))
}

/// Read the exponent of a native or significant-decimal observation.
fn exponent(text: &str) -> i32 {
    text.rsplit('e').next().unwrap_or("0").parse().unwrap_or(0)
}

/// Derive engineering displacement from rounded significant digits, including carry.
fn engineering(value: f64, period: usize) -> String {
    if value == 0.0 {
        return "0.0E+0".to_owned();
    }
    let period = period.to_i32().unwrap_or(i32::MAX);
    let original = exponent(&format!("{:e}", value.abs()));
    let count = (original.rem_euclid(period) + 2).to_usize().unwrap_or(2);
    let text = super::general::precision(value.abs(), count, true);
    let rounded = exponent(&text);
    let places = (rounded.rem_euclid(period) + 1).to_usize().unwrap_or(1);
    let digits = text.split('e').next().unwrap_or(&text).replace('.', "");
    let digits = format!("{digits:0<width$}", width = places + 1);
    let displacement = rounded - rounded.rem_euclid(period);
    format!(
        "{}{}.{}E{displacement:+}",
        sign(value),
        &digits[..places],
        &digits[places..=places]
    )
}

/// Test a nonempty ASCII placeholder field against its admitted vocabulary.
fn field(text: &str, allowed: &str) -> bool {
    !text.is_empty() && text.bytes().all(|c| allowed.as_bytes().contains(&c))
}

/// Numeric sign outside branches that round the signed observation.
fn sign(value: f64) -> &'static str {
    if value < 0.0 { "-" } else { "" }
}
