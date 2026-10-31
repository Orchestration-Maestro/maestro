//! Summary generation reports the provider usage beside its text.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::scripted_summary::{ScriptedSummary, options, usage};
use maestro_session::{generate_summary, generate_summary_with_usage};
use serde_json::json;

/// Complete `future` on a fresh current-thread runtime.
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(future)
}

#[test]
fn summary_with_usage_returns_text_and_provider_usage() {
    let reported = usage(11, 7, 0.5);
    let (provider, model) = ScriptedSummary::register(vec![("## Goal\nSummary", reported.clone())]);
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let summary = block_on(generate_summary_with_usage(
        &messages,
        &model,
        2000.0,
        None,
        &options(),
    ))
    .unwrap();
    assert_eq!(summary.text, "## Goal\nSummary");
    let expected: maestro_models::Usage = serde_json::from_value(reported).unwrap();
    assert_eq!(summary.usage, expected);
    assert_eq!(provider.remaining(), 0);
}

#[test]
fn text_only_summary_keeps_returning_text() {
    let (_provider, model) =
        ScriptedSummary::register(vec![("## Goal\nSummary", usage(11, 7, 0.5))]);
    let messages = [json!({"role": "user", "content": "work", "timestamp": 1})];
    let text = block_on(generate_summary(
        &messages,
        &model,
        2000.0,
        None,
        &options(),
    ))
    .unwrap();
    assert_eq!(text, "## Goal\nSummary");
}
