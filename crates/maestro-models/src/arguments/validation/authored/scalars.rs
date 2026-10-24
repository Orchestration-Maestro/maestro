//! Declared scalar conversions with the supplied JSON values they accept.

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde_json::{Number, Value};

use super::super::coercion;

/// Largest integer magnitude a JSON number represents exactly.
const SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Convert text, numbers, truth values and null through the declared string kind.
pub(super) fn to_string(value: &mut Value) {
    let converted = match &*value {
        Value::Bool(boolean) => Value::String(boolean.to_string()),
        Value::Number(number) => match coercion::number_text(number) {
            Some(text) => Value::String(text),
            None => return,
        },
        Value::Null => Value::String("null".into()),
        _ => return,
    };
    *value = converted;
}

/// Convert through the declared number kind, truncating for an integer declaration.
pub(super) fn to_number(value: &mut Value, integer: bool) {
    let Some(converted) = number(value) else {
        return;
    };
    if !integer {
        *value = converted;
        return;
    }
    let Some(number) = converted.as_f64() else {
        return;
    };
    if let Some(truncated) = Number::from_f64(number.trunc()) {
        *value = Value::Number(truncated);
    }
}

/// Convert through the declared boolean kind.
pub(super) fn to_boolean(value: &mut Value) {
    let converted = match &*value {
        Value::Number(number) => match number.as_f64() {
            Some(0.0) => Value::Bool(false),
            Some(1.0) => Value::Bool(true),
            _ => return,
        },
        Value::Null => Value::Bool(false),
        Value::String(text) => match text.as_str() {
            "0" => Value::Bool(false),
            "1" => Value::Bool(true),
            _ if text.to_lowercase() == "false" => Value::Bool(false),
            _ if text.to_lowercase() == "true" => Value::Bool(true),
            _ => return,
        },
        _ => return,
    };
    *value = converted;
}

/// Convert through the declared null kind.
pub(super) fn to_null(value: &mut Value) {
    let converted = match &*value {
        Value::Bool(false) => Value::Null,
        Value::Number(number) if number.as_f64() == Some(0.0) => Value::Null,
        Value::String(text) => match text.as_str() {
            "" | "0" => Value::Null,
            _ if text.to_lowercase() == "null" || text.to_lowercase() == "undefined" => Value::Null,
            _ => return,
        },
        _ => return,
    };
    *value = converted;
}

/// Wrap any supplied value into a sequence, keeping sequences unchanged.
pub(super) fn to_array(value: Value) -> Value {
    match value {
        Value::Array(_) => value,
        other => Value::Array(vec![other]),
    }
}

/// Convert a supplied value into a finite number, when the number kind admits it.
fn number(value: &Value) -> Option<Value> {
    let converted = match value {
        Value::Bool(boolean) => f64::from(u8::from(*boolean)),
        Value::Number(number) => return Some(Value::Number(number.clone())),
        Value::Null => 0.0,
        Value::String(text) => number_text(text)?,
        _ => return None,
    };
    Number::from_f64(converted).map(Value::Number)
}

/// Read text as a number the way the supplied scalar rules do.
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
