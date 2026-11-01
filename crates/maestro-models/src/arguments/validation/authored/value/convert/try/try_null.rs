//! Conversion of a supplied value to the JSON null value.

use serde_json::Value;

/// Convert the supplied value to null, or `None` when the reference attempt fails.
pub(crate) fn try_null(value: &Value) -> Option<Value> {
    let converted = match value {
        Value::Bool(false) | Value::Null => true,
        Value::Number(number) => number.as_f64() == Some(0.0),
        Value::String(text) => match text.as_str() {
            "" | "0" => true,
            _ => text.to_lowercase() == "null" || text.to_lowercase() == "undefined",
        },
        Value::Bool(true) | Value::Array(_) | Value::Object(_) => false,
    };
    converted.then_some(Value::Null)
}
