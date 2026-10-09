//! JSON value comparison helpers shared by the protocol tests.

use serde_json::Value;

/// Rewrite integral floats as integers so values compare as their JSON text would.
pub fn canonical(value: Value) -> Value {
    match value {
        Value::Number(number) => number
            .as_f64()
            .filter(|float| number.is_f64() && float.fract() == 0.0 && float.abs() < 9.0e15)
            .and_then(|float| format!("{float}").parse::<serde_json::Number>().ok())
            .map_or(Value::Number(number), Value::Number),
        Value::Array(items) => Value::Array(items.into_iter().map(canonical).collect()),
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .map(|(k, v)| (k, canonical(v)))
                .collect(),
        ),
        other => other,
    }
}
