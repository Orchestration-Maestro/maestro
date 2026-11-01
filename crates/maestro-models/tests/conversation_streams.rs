//! Direct reasoning conversation stream behavior.
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::reasoning::mistral::stream_simple_mistral;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::stream_mistral;
#[cfg(not(target_arch = "wasm32"))]
use serde_json::json;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_keep_raw_and_simple_failure_boundaries() -> TestResult {
    let model = chat::model(&json!({"provider": "oracle-no-env", "api": "mistral-conversations"}))?;
    let context = chat::context(&json!({"messages": []}))?;
    assert_eq!(
        stream_simple_mistral(model.clone(), context.clone(), None)
            .err()
            .unwrap()
            .message,
        "No API key for provider: oracle-no-env"
    );
    lifecycle::no_runtime()?;
    crate::runtime::run(async {
        lifecycle::dropped_reader().await?;
        lifecycle::simple_controls().await?;
        lifecycle::mapped_simple_controls().await?;
        lifecycle::raw_whitespace_key().await?;
        let stream = stream_mistral(model, context, None);
        let reader = stream.clone();
        let event = reader.next().await.unwrap();
        assert!(matches!(
            event,
            maestro_models::AssistantMessageEvent::Error { .. }
        ));
        assert!(reader.next().await.is_none());
        let result = stream.result().await;
        assert_eq!(
            result.read().unwrap().error_message.as_deref(),
            Some("No API key for provider: oracle-no-env")
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "conversation_streams/corpus.rs"]
mod corpus;

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_share_message_updates_after_admission() -> TestResult {
    crate::runtime::run(async {
        corpus::rows("conversation_streams_share_message_updates_after_admission").await?;
        lifecycle::headers_gate().await?;
        lifecycle::shared_admission().await?;
        lifecycle::refused_admission().await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_retain_first_id_and_usage() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_retain_first_id_and_usage",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_map_finish_outcomes() -> TestResult {
    crate::runtime::run(corpus::rows("conversation_streams_map_finish_outcomes"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_order_text_and_thinking_blocks() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_order_text_and_thinking_blocks",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_arguments_keep_encoded_root_shapes() -> TestResult {
    crate::runtime::run(async {
        corpus::rows("conversation_arguments_keep_encoded_root_shapes").await?;
        corpus::recorded_roots().await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_preserve_partial_results_on_failure() -> TestResult {
    crate::runtime::run_paused(async {
        corpus::rows("conversation_streams_preserve_partial_results_on_failure").await?;
        lifecycle::lifetime_failures().await?;
        lifecycle::configured_timeout().await?;
        lifecycle::failure_causes().await?;
        lifecycle::exhausted_signal().await
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_respect_framing_and_completion() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_respect_framing_and_completion",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_streams_validate_complete_events_before_reduction() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_validate_complete_events_before_reduction",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn conversation_error_finish_reports_a_retryable_server_error() -> TestResult {
    use maestro_models::{SimpleStreamOptions, StreamOptions};
    crate::runtime::run(async {
        let chunk = br#"data: {"id":"r","model":"m","choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":"error"}]}

"#;
        let stream = stream_simple_mistral(
            corpus::model()?,
            chat::context(&json!({"messages": []}))?,
            Some(SimpleStreamOptions {
                common: StreamOptions {
                    api_key: Some("fixture-key".into()),
                    fetch: Some(corpus::fetch(vec![chunk.to_vec()])),
                    ..Default::default()
                },
                ..Default::default()
            }),
        )?;
        let result = stream.result().await;
        let result = result.read().unwrap();
        assert_eq!(result.stop_reason, maestro_models::StopReason::Error);
        assert_eq!(
            result.error_message.as_deref(),
            Some("Provider stopped with: error (server error)")
        );
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[path = "conversation_streams/lifecycle.rs"]
mod lifecycle;
