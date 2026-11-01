//! Controlled public message qualification.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::message_qualification as qualification;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Simple catalog mappings and raw effort/display controls retain distinct values.
#[test]
fn maestro_message_preserves_adaptive_effort() -> TestResult {
    qualification::assert_rows("maestro_message_preserves_adaptive_effort")
}

#[cfg(not(target_arch = "wasm32"))]
/// Ordered thinking, accumulated signature and exact arithmetic bytes survive streaming.
#[test]
fn maestro_message_streams_signed_arithmetic() -> TestResult {
    qualification::assert_rows("maestro_message_streams_signed_arithmetic")
}

#[cfg(not(target_arch = "wasm32"))]
/// A Messages compatibility flag selects adaptive thinking independently of the model id.
#[test]
fn messages_force_adaptive_thinking_uses_effort_without_a_budget() -> TestResult {
    use maestro_models::{SimpleStreamOptions, StreamOptions, ThinkingLevel, stream_simple};
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};

    crate::runtime::run(async {
        for (level, expected) in [
            (ThinkingLevel::Minimal, "low"),
            (ThinkingLevel::Low, "low"),
            (ThinkingLevel::Medium, "medium"),
            (ThinkingLevel::High, "high"),
            (ThinkingLevel::Xhigh, "high"),
        ] {
            let requests = Arc::new(Mutex::new(Vec::new()));
            let model = crate::messages::model(&json!({
                "id": "claude-sonnet-5", "compat": {"forceAdaptiveThinking": true}
            }))?;
            let options = SimpleStreamOptions {
                common: StreamOptions {
                    api_key: Some("fixture-key".into()),
                    fetch: Some(crate::messages::scripted_fetch(
                        &crate::messages::Case::default(),
                        &requests,
                    )),
                    ..StreamOptions::default()
                },
                reasoning: Some(level),
                ..SimpleStreamOptions::default()
            };
            let stream = stream_simple(
                model,
                crate::messages::context(&Value::Null)?,
                Some(options),
            )?;
            crate::messages::collect(&stream).await?;
            let requests = requests.lock().unwrap();
            let request = &requests[0];
            assert_eq!(
                request["body"]["thinking"],
                json!({"type": "adaptive", "display": "summarized"})
            );
            assert_eq!(
                request["body"]["output_config"],
                json!({"effort": expected})
            );
            assert_eq!(request["headers"]["anthropic-beta"], Value::Null);
        }
        Ok(())
    })
}
