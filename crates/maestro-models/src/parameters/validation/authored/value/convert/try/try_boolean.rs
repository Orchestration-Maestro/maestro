//! Conversion of a supplied value to a truth value.

use serde_json::Value;

/// Convert the supplied value to a truth value, or `None` when the reference attempt fails.
pub(crate) fn try_boolean(value: &Value) -> Option<Value> {
    let converted = match value {
        Value::Bool(boolean) => *boolean,
        Value::Number(number) => match number.as_f64() {
            Some(0.0) => false,
            Some(1.0) => true,
            _ => return None,
        },
        Value::Null => false,
        Value::String(text) => match text.to_lowercase().as_str() {
            "0" | "false" => false,
            "1" | "true" => true,
            _ => return None,
        },
        Value::Array(_) | Value::Object(_) => return None,
    };
    Some(Value::Bool(converted))
}
