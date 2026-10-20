//! Direct reasoning conversation stream behavior.
use crate::chat;

use chat::TestResult;
use maestro_models::providers::reasoning::mistral::stream_simple_mistral;
use maestro_models::stream_mistral;
use serde_json::json;

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

#[path = "conversation_streams/corpus.rs"]
mod corpus;

#[test]
fn conversation_streams_share_message_updates_after_admission() -> TestResult {
    crate::runtime::run(async {
        corpus::rows("conversation_streams_share_message_updates_after_admission").await?;
        lifecycle::headers_gate().await?;
        lifecycle::shared_admission().await?;
        lifecycle::refused_admission().await
    })
}

#[test]
fn conversation_streams_retain_first_id_and_usage() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_retain_first_id_and_usage",
    ))
}

#[test]
fn conversation_streams_map_finish_outcomes() -> TestResult {
    crate::runtime::run(corpus::rows("conversation_streams_map_finish_outcomes"))
}

#[test]
fn conversation_streams_order_text_and_thinking_blocks() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_order_text_and_thinking_blocks",
    ))
}

#[test]
fn conversation_streams_accumulate_and_finalize_tool_calls() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_accumulate_and_finalize_tool_calls",
    ))
}

#[test]
fn conversation_streams_preserve_partial_results_on_failure() -> TestResult {
    crate::runtime::run_paused(async {
        corpus::rows("conversation_streams_preserve_partial_results_on_failure").await?;
        lifecycle::lifetime_failures().await?;
        lifecycle::failure_causes().await?;
        lifecycle::exhausted_signal().await
    })
}

#[test]
fn conversation_streams_respect_framing_and_completion() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_respect_framing_and_completion",
    ))
}

#[test]
fn conversation_streams_validate_complete_events_before_reduction() -> TestResult {
    crate::runtime::run(corpus::rows(
        "conversation_streams_validate_complete_events_before_reduction",
    ))
}

#[path = "conversation_streams/lifecycle.rs"]
mod lifecycle;
