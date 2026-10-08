//! Fixtures, model builders and request capture shared by chat-completion tests.

use maestro_models::{Context, Model};
use serde_json::{Value, json};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Drive a future to completion on a fresh single-threaded runtime.
///
/// With `paused` set, timers advance only when every task is waiting, so waits take no real time.
pub fn block_on<T>(
    paused: bool,
    future: impl std::future::Future<Output = TestResult<T>>,
) -> TestResult<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(paused)
        .build()?
        .block_on(future)
}

/// Rows of a fixture file owned by one test.
pub fn rows(fixture: &str, test: &str) -> TestResult<Vec<Value>> {
    let all: Vec<Value> = serde_json::from_str(fixture)?;
    Ok(all.into_iter().filter(|row| row["test"] == test).collect())
}

/// Merge `overrides` over the shared fixture model and decode it.
pub fn model(overrides: &Value) -> TestResult<Model> {
    let mut value = json!({
        "id": "model", "name": "Model", "api": "openai-completions", "provider": "fixture",
        "baseUrl": "https://api.openai.com/v1", "reasoning": true, "input": ["text", "image"],
        "cost": {"input": 1, "output": 2, "cacheRead": 0.1, "cacheWrite": 1.25},
        "contextWindow": 128_000, "maxTokens": 4096
    });
    if let (Some(base), Some(changes)) = (value.as_object_mut(), overrides.as_object()) {
        base.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    Ok(serde_json::from_value(value)?)
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
