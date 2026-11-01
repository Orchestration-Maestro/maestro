//! Complete-session compaction setup and observations over the scripted provider.
#![cfg(not(target_arch = "wasm32"))]
use super::session_harness::{Harness, HarnessOptions, Scripted, harness, start_prompt};
use maestro_session::CompactionResult;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};

/// Builds two completed turns, retaining one token's worth of recent history.
pub async fn seeded(mut options: HarnessOptions) -> Harness {
    options.context_window = Some(200_000.0);
    options.settings = json!({"compaction": {"keepRecentTokens": 1}, "retry": {"enabled": false}});
    options
        .responses
        .get_or_insert_with(|| vec![Scripted::text("one"), Scripted::text("two")]);
    let h = harness(options).await;
    for text in ["first", "second"] {
        start_prompt(&h, text).await.unwrap().unwrap();
    }
    h
}

/// Asserts both exact summary requests, independent of concurrent request scheduling.
pub fn assert_prompts(actual: &[String], expected: &Value) {
    let mut expected: Vec<&str> = expected["requests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|request| {
            request["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
        })
        .collect();
    let mut actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

/// Checks the summary, token estimate, details and selected entry against the recording.
pub fn assert_result(h: &Harness, result: &CompactionResult, expected: &Value) {
    assert_eq!(result.summary, expected["summary"]);
    assert_eq!(
        result.tokens_before.to_bits(),
        expected["tokens_before"].as_f64().unwrap().to_bits()
    );
    assert_eq!(
        serde_json::to_value(&result.details).unwrap(),
        expected["details"]
    );
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    let entries = manager.get_entries();
    let index = entries
        .iter()
        .position(|entry| entry["id"] == result.first_kept_entry_id)
        .unwrap();
    assert_eq!(json!(index), expected["first_kept_index"]);
}

/// The live messages in their stored interchange form.
pub fn messages(h: &Harness) -> Vec<Value> {
    serde_json::from_value(serde_json::to_value(&*h.session.messages().read().unwrap()).unwrap())
        .unwrap()
}
