//! Conversion of a supplied value to a number.

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde_json::{Number, Value};

use crate::arguments::validation::coercion;

/// Largest integer magnitude a JSON number represents exactly.
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Convert the supplied value to a number, or `None` when the reference attempt fails.
pub(crate) fn try_number(value: &Value) -> Option<Value> {
    let converted = match value {
        Value::Bool(boolean) => f64::from(u8::from(*boolean)),
        Value::Number(number) => return Some(Value::Number(number.clone())),
        Value::Null => 0.0,
        Value::String(text) => number_text(text)?,
        Value::Array(_) | Value::Object(_) => return None,
    };
    Number::from_f64(converted).map(Value::Number)
}

/// Convert the supplied value to a number without a fractional part.
pub(crate) fn try_integer(value: &Value) -> Option<Value> {
    let converted = try_number(value)?;
    let number = converted.as_f64()?;
    Number::from_f64(number.trunc()).map(Value::Number)
}

/// Read text as a number the way the reference numeric attempt does.
fn number_text(text: &str) -> Option<f64> {
    if coercion::parse_number(text).is_none() {
        let trimmed = text.trim_matches(crate::arguments::json_parse::whitespace);
        if !trimmed.is_empty() {
            return bigint(trimmed)
                .filter(|number| number.abs() <= SAFE_INTEGER)
                .or_else(|| word(text));
        }
        return Some(0.0);
    }
    coercion::parse_number(text)
}

/// Read the truth words as their numeric values.
fn word(text: &str) -> Option<f64> {
    match text.to_lowercase().as_str() {
        "false" => Some(0.0),
        "true" => Some(1.0),
        _ => None,
    }
}

/// Read integer-suffix text as a whole number.
fn bigint(text: &str) -> Option<f64> {
    let digits = text.strip_suffix('n')?;
    let negative = digits.starts_with('-');
    let digits = digits
        .strip_prefix('-')
        .or_else(|| digits.strip_prefix('+'))
        .unwrap_or(digits);
    if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
        return None;
    }
    let magnitude = digits.parse::<BigInt>().ok()?;
    let magnitude = if negative { -magnitude } else { magnitude };
    magnitude.to_f64()
}
