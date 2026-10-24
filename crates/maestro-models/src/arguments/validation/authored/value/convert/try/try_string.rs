//! Conversion of a supplied value to text.

use serde_json::Value;

use crate::arguments::validation::coercion;

/// Convert the supplied value to text, or `None` when the reference attempt fails.
pub(crate) fn try_string(value: &Value) -> Option<Value> {
    match value {
        Value::Bool(boolean) => Some(Value::String(boolean.to_string())),
        Value::Number(number) => coercion::number_text(number).map(Value::String),
        Value::Null => Some(Value::String("null".into())),
        Value::String(text) => Some(Value::String(text.clone())),
        Value::Array(_) | Value::Object(_) => None,
    }
}
