//! Shared assistant state and native launch policy through public surfaces.

use crate::chat;
use maestro_models::providers::assistant_output::initial_message;
use maestro_models::{AssistantMessage, timestamp_now};
use serde_json::Value;

#[test]
fn assistant_initial_state_matches_recorded_fields() -> chat::TestResult {
    let rows: Vec<Value> = serde_json::from_str(include_str!("fixtures/assistant-output.json"))?;
    let mut count = 0;
    for row in rows.iter().filter(|row| row["kind"] == "initial") {
        assert!(
            [
                "chat",
                "messages",
                "responses",
                "sessions",
                "conversation",
                "content",
                "azure",
                "bedrock"
            ]
            .contains(&row["input"]["owner"].as_str().unwrap())
        );
        let model = chat::model(&row["input"]["model"])?;
        let before = timestamp_now();
        let mut actual = initial_message(&model);
        assert!((before..=timestamp_now()).contains(&actual.timestamp));
        actual.timestamp = row["input"]["timestamp"].as_f64().unwrap();
        let expected: AssistantMessage = serde_json::from_value(row["expected"].clone())?;
        if matches!(row["input"]["owner"].as_str(), Some("content" | "bedrock")) {
            // Generic initialization keeps the supplied API; protocol streams choose their own.
            assert_eq!(actual.api, model.api);
            assert_eq!(
                expected.api,
                if row["input"]["owner"] == "content" {
                    "google-generative-ai"
                } else {
                    "bedrock-converse-stream"
                }
            );
        } else {
            assert_eq!(actual.api, expected.api);
        }
        assert_initial_shared_fields(actual, expected);
        count += 1;
    }
    assert_eq!(count, 16);
    Ok(())
}

/// Compare the complete shared record except protocol-owned API identity.
fn assert_initial_shared_fields(actual: AssistantMessage, expected: AssistantMessage) {
    assert_eq!(
        (
            actual.content,
            actual.provider,
            actual.model,
            actual.response_model,
            actual.response_id,
            actual.diagnostics,
            actual.usage,
            actual.stop_reason,
            actual.error_message,
            actual.timestamp
        ),
        (
            expected.content,
            expected.provider,
            expected.model,
            expected.response_model,
            expected.response_id,
            expected.diagnostics,
            expected.usage,
            expected.stop_reason,
            expected.error_message,
            expected.timestamp
        )
    );
}
