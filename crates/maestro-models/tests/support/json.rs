//! JSON value comparison and decoding helpers shared by the protocol tests.

use crate::chat::TestResult;
use maestro_models::Context;
use serde::Deserialize as _;
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

/// Decode the optional member a case names.
///
/// # Errors
/// Returns the decoding failure when the member is present but malformed.
pub fn decode<T: serde::de::DeserializeOwned>(spec: &Value, key: &str) -> TestResult<Option<T>> {
    Ok(spec
        .get(key)
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()?)
}

/// Decode a fixture context only when every field survives in the public type.
///
/// # Errors
/// Returns the decoding failure, or a failure when the public type drops a fixture field.
pub fn exact_context<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Context, D::Error> {
    use serde::de::Error as _;
    let value = Value::deserialize(decoder)?;
    let context: Context = serde_json::from_value(value.clone()).map_err(D::Error::custom)?;
    let retained = serde_json::to_value(&context).map_err(D::Error::custom)?;
    if canonical(retained) != canonical(value) {
        return Err(D::Error::custom("context fixture fields were not retained"));
    }
    Ok(context)
}
