//! Protocol fixture row selection and conversation defaults.
pub use crate::descriptors::{TestResult, model, overlay};
use maestro_models::Context;
use serde_json::{Value, json};
/// Rows of a fixture file owned by one test.
pub fn rows(fixture: &str, test: &str) -> TestResult<Vec<Value>> {
    let all: Vec<Value> = serde_json::from_str(fixture)?;
    Ok(all.into_iter().filter(|row| row["test"] == test).collect())
}

/// Fill the defaults fixtures omit from messages, then decode the context.
pub fn context(compact: &Value) -> TestResult<Context> {
    let mut value = compact.clone();
    let messages = value
        .get_mut("messages")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten();
    for message in messages {
        let defaults = match message["role"].as_str() {
            Some("assistant") => json!({
                "api": "openai-completions", "provider": "fixture", "model": "model",
                "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0,
                    "totalTokens": 0, "cost": {"input": 0, "output": 0, "cacheRead": 0,
                    "cacheWrite": 0, "total": 0}},
                "stopReason": "stop", "timestamp": 2
            }),
            Some("user") => json!({"timestamp": 1}),
            _ => json!({"isError": false, "timestamp": 3}),
        };
        if let (Some(target), Some(defaults)) = (message.as_object_mut(), defaults.as_object()) {
            for (key, default) in defaults {
                target.entry(key.clone()).or_insert_with(|| default.clone());
            }
        }
    }
    Ok(serde_json::from_value(value)?)
}
