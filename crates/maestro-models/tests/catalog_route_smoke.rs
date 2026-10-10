//! The literal gateway catalogs route to the bundled adapters.
#![allow(
    dead_code,
    reason = "each test binary uses a subset of the shared support"
)]

#[path = "support/bundled.rs"]
mod bundled;
#[path = "support/chat.rs"]
mod chat;

use chat::{TestResult, block_on};
use maestro_models::{
    ProviderStreamOptions, StopReason, StreamOptions, complete, get_models, reset_api_providers,
};

const OPENCODE: [&str; 38] = [
    "big-pickle",
    "claude-haiku-4-5",
    "claude-opus-4-1",
    "claude-opus-4-5",
    "claude-opus-4-6",
    "claude-opus-4-7",
    "claude-sonnet-4",
    "claude-sonnet-4-5",
    "claude-sonnet-4-6",
    "gemini-3-flash",
    "gemini-3.1-pro",
    "glm-5",
    "glm-5.1",
    "gpt-5",
    "gpt-5-codex",
    "gpt-5-nano",
    "gpt-5.1",
    "gpt-5.1-codex",
    "gpt-5.1-codex-max",
    "gpt-5.1-codex-mini",
    "gpt-5.2",
    "gpt-5.2-codex",
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.4-nano",
    "gpt-5.4-pro",
    "gpt-5.5",
    "gpt-5.5-pro",
    "hy3-preview-free",
    "kimi-k2.5",
    "kimi-k2.6",
    "minimax-m2.5",
    "minimax-m2.5-free",
    "minimax-m2.7",
    "nemotron-3-super-free",
    "qwen3.5-plus",
    "qwen3.6-plus",
];

const OPENCODE_GO: [&str; 14] = [
    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "glm-5",
    "glm-5.1",
    "kimi-k2.5",
    "kimi-k2.6",
    "mimo-v2-omni",
    "mimo-v2-pro",
    "mimo-v2.5",
    "mimo-v2.5-pro",
    "minimax-m2.5",
    "minimax-m2.7",
    "qwen3.5-plus",
    "qwen3.6-plus",
];

/// Content-generation descriptors, which no registered adapter serves yet.
const DEFERRED: [&str; 2] = ["gemini-3-flash", "gemini-3.1-pro"];

#[test]
fn maestro_catalog_routes_use_available_adapters() -> TestResult {
    let _registry = bundled::registry();
    reset_api_providers();
    let mut completed = 0;
    let mut deferred = Vec::new();
    for (provider, expected) in [
        ("opencode", OPENCODE.to_vec()),
        ("opencode-go", OPENCODE_GO.to_vec()),
    ] {
        let models = get_models(provider);
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, expected, "{provider} inventory");
        for model in models {
            if !bundled::apis().contains(&model.api.as_str()) {
                deferred.push(model.id.clone());
                continue;
            }
            let text = format!("{provider}/{}", model.id);
            let (fetch, requests) = bundled::fetch(bundled::text_body(&model.api, &text));
            let options = ProviderStreamOptions {
                common: StreamOptions {
                    api_key: Some("controlled-key".into()),
                    fetch: Some(fetch),
                    ..StreamOptions::default()
                },
                ..ProviderStreamOptions::default()
            };
            let id = model.id.clone();
            let result = block_on(false, async {
                Ok(complete(model, bundled::conversation()?, Some(options)).await?)
            })?;
            assert_eq!(
                bundled::stop_of(&result),
                StopReason::Stop,
                "{text}: {:?}",
                bundled::error_of(&result)
            );
            assert_eq!(bundled::text_of(&result), text);
            assert_eq!(bundled::recorded(&requests).len(), 1, "{id}");
            completed += 1;
        }
    }
    assert_eq!(completed, 50);
    assert_eq!(deferred, DEFERRED);
    Ok(())
}
