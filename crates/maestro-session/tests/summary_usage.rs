//! Summary generation reports the provider usage beside its text.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::generation::answering;
use maestro_session::{generate_summary, generate_summary_with_usage};
use serde_json::{Value, json};

/// Complete `future` on a fresh current-thread runtime.
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

/// Usage the provider reports for the scripted request.
fn usage() -> Value {
    json!({"input": 11, "output": 7, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 18,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0.5}})
}

#[test]
fn summary_with_usage_returns_text_and_provider_usage() {
    let reported = usage();
    let (_adapter, model, options) = answering("## Goal\nSummary", &reported);
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let summary = block_on(generate_summary_with_usage(
        &messages, &model, 2000.0, None, &options,
    ))
    .unwrap();
    assert_eq!(summary.text, "## Goal\nSummary");
    let expected: maestro_models::Usage = serde_json::from_value(reported).unwrap();
    assert_eq!(summary.usage, expected);
}

#[test]
fn text_only_summary_keeps_returning_text() {
    let (_adapter, model, options) = answering("## Goal\nSummary", &usage());
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let text = block_on(generate_summary(&messages, &model, 2000.0, None, &options)).unwrap();
    assert_eq!(text, "## Goal\nSummary");
}
