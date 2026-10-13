//! Whether a JSON value counts as present when used as a condition.

use serde_json::Value;
use serde_json::value::RawValue;

/// Report whether a represented value is truthy.
///
/// Null, `false`, zero of either sign and the empty string are not; every other string,
/// array and object is. Callers decide absence and nullish defaults themselves.
#[must_use]
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64() != Some(0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Report whether valid raw JSON is truthy by the rules of [`is_truthy`], reading no
/// container. A number beyond the range of a double is an infinity and so truthy, and
/// one below it is zero and so not.
#[must_use]
pub fn is_raw_truthy(value: &RawValue) -> bool {
    match value.get() {
        "null" | "false" | r#""""# => false,
        text => text.parse::<f64>() != Ok(0.0),
    }
}
