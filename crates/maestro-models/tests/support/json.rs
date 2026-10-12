//! JSON value comparison helpers shared by the protocol tests.
#![allow(dead_code, reason = "Each test binary uses part of the JSON helpers.")]

use serde_json::Value;

/// Serialize ordered event data without the live message handles.
///
/// # Errors
/// Returns the serializer failure, or a failure when the event is not an object.
pub fn event_record(
    event: &maestro_models::AssistantMessageEvent,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut record = serde_json::to_value(event)?;
    let object = record.as_object_mut().ok_or("event object")?;
    for field in ["partial", "message", "error"] {
        object.shift_remove(field);
    }
    Ok(record)
}

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
