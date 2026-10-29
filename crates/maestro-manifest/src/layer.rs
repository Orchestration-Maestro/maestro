//! The settings owner's base-layer record and the load-time payload that carries it.

use serde_json::Value;

use crate::areas::Composition;

/// One layer in the settings owner's record form, naming its source release and when it was accepted.
#[must_use]
pub fn record(_composition: &Composition, _source: &str, _recorded_at: &str) -> Value {
    Value::Null
}

/// The load-time contribution that publishes a layer per scope, or removes the scope's layer.
#[must_use]
pub fn payload(_layers: &[(&str, Option<Value>)]) -> Value {
    Value::Null
}

/// The current UTC time in RFC 3339 form, to the second.
#[must_use]
pub fn recorded_at() -> String {
    String::new()
}
