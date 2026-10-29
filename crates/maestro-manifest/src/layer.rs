//! The settings owner's base-layer record and the load-time payload that carries it.

use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};

use crate::areas::Composition;

/// One layer in the settings owner's record form, naming its source release and when it was accepted.
#[must_use]
pub fn record(composition: &Composition, source: &str, recorded_at: &str) -> Value {
    json!({
        "settings": composition.settings,
        "locked": composition.locked,
        "source": source,
        "recorded_at": recorded_at
    })
}

/// The load-time contribution that publishes a layer per scope, or removes the scope's layer.
#[must_use]
pub fn payload(layers: &[(&str, Option<Value>)]) -> Value {
    let layers: Vec<Value> = layers
        .iter()
        .map(|(scope, layer)| json!({"scope": scope, "layer": layer}))
        .collect();
    json!({ "baseLayers": layers })
}

/// The current UTC time in RFC 3339 form, to the second.
#[must_use]
pub fn recorded_at() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}
