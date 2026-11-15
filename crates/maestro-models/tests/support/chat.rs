//! Fixtures, model builders and request capture shared by chat-completion tests.

use maestro_models::Model;
use serde_json::{Value, json};

/// Fallible test body result.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// `base` with the members of `overrides` replacing its own.
pub fn overlay(mut base: Value, overrides: &Value) -> Value {
    if let (Some(members), Some(changes)) = (base.as_object_mut(), overrides.as_object()) {
        members.extend(changes.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    base
}

/// Merge `overrides` over the shared fixture model and decode it.
pub fn model(overrides: &Value) -> TestResult<Model> {
    let value = json!({
        "id": "model", "name": "Model", "api": "openai-completions", "provider": "fixture",
        "baseUrl": "https://api.openai.com/v1", "reasoning": true, "input": ["text", "image"],
        "cost": {"input": 1, "output": 2, "cacheRead": 0.1, "cacheWrite": 1.25},
        "contextWindow": 128_000, "maxTokens": 4096
    });
    Ok(serde_json::from_value(overlay(value, overrides))?)
}
