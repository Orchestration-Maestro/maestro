//! Protocol fixture row selection and conversation defaults.
pub use crate::descriptors::{TestResult, model, overlay};
use maestro_models::Context;
use serde_json::{Value, json};
/// Returns fixture rows assigned to `test`, preserving their order.
///
/// # Errors
/// Returns an error if the fixture cannot be decoded or no row matches `test`.
pub fn rows(fixture: &str, test: &str) -> TestResult<Vec<Value>> {
    let all: Vec<Value> = serde_json::from_str(fixture)?;
    let selected: Vec<_> = all.into_iter().filter(|row| row["test"] == test).collect();
    if selected.is_empty() {
        return Err(format!("no fixture rows for {test:?}").into());
    }
    Ok(selected)
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

#[cfg(test)]
mod tests {
    //! Fixture selection rejects keys that would leave HTTP checks unexercised.
    use super::*;

    #[test]
    fn fixture_rows_reject_unknown_keys() {
        let fixture = include_str!("../fixtures/chat_completions/http.json");
        for key in [
            "http_joins_endpoint_urls_missing",
            "http_preserves_setup_timeout_missing",
        ] {
            let error = rows(fixture, key).expect_err("an unknown key must not skip assertions");
            assert!(error.to_string().contains(key), "{error}");
        }
    }
}
