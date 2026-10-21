//! Invocation and event-consumption examples.

#[cfg(not(target_arch = "wasm32"))]
use super::response_options;
#[cfg(not(target_arch = "wasm32"))]
use crate::{bundled, chat::TestResult};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    ProviderStreamOptions, SimpleStreamOptions, StopReason, complete, complete_simple, get_model,
    stream, stream_simple,
};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_guide_quick_start_completes_without_network() -> TestResult {
    let _registry = bundled::registry();
    crate::runtime::run(async {
        let model = get_model("openai", "gpt-4o-mini").ok_or("catalog model")?;
        let context = bundled::conversation()?;
        let (common, requests) = response_options(&model.api, "Hello");
        let raw = ProviderStreamOptions {
            common: common.clone(),
            ..Default::default()
        };
        let simple = SimpleStreamOptions {
            common,
            ..Default::default()
        };
        let raw_events = stream(model.clone(), context.clone(), Some(raw.clone()))?;
        let simple_events = stream_simple(model.clone(), context.clone(), Some(simple.clone()))?;
        for events in [raw_events, simple_events] {
            check_quick_stream(events).await?;
        }
        check_quick_result(&complete(model.clone(), context.clone(), Some(raw)).await?)?;
        check_quick_result(&complete_simple(model, context, Some(simple)).await?)?;
        assert_eq!(bundled::recorded(&requests).len(), 4);
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// The same observable answer and reported usage through each invocation route.
fn check_quick_result(result: &maestro_models::SharedAssistantMessage) -> TestResult {
    assert_eq!(bundled::error_of(result), None);
    assert_eq!(bundled::text_of(result), "Hello");
    assert_eq!(bundled::stop_of(result), StopReason::Stop);
    let message = result.read().map_err(|_| "message lock")?;
    let usage = crate::json::canonical(serde_json::to_value(&message.usage)?);
    assert_eq!(usage["input"], 3);
    assert_eq!(usage["output"], 2);
    assert_eq!(usage["totalTokens"], 5);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Native message frames with three independent content indices.
fn interleaved_frames() -> Vec<u8> {
    use serde_json::json;
    [
        json!({"type":"message_start","message":{"id":"m","usage":{"input_tokens":3,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_start","index":2,"content_block":{"type":"tool_use","id":"weather-1","name":"get_weather","input":{}}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"plan"}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Checking"}}),
        json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" now"}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":" weather"}}),
        json!({"type":"content_block_delta","index":2,"delta":{"type":"input_json_delta","partial_json":"\"London\"}"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"content_block_stop","index":1}),
        json!({"type":"content_block_stop","index":2}),
        json!({"type":"message_delta","delta":{"stop_reason":"tool_use"},"usage":{"output_tokens":2}}),
        json!({"type":"message_stop"}),
    ].iter().map(crate::messages::sse).collect::<String>().into_bytes()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_guide_consumes_interleaved_stream() -> TestResult {
    use maestro_models::{
        AnthropicOptions, AssistantMessageEvent, StreamOptions, stream_anthropic,
    };
    use std::sync::Arc;
    let _registry = bundled::registry();
    crate::runtime::run(async {
        let (fetch, _) = bundled::fetch(interleaved_frames());
        let model = crate::messages::model(&serde_json::json!({}))?;
        let events = stream_anthropic(
            model,
            bundled::conversation()?,
            Some(AnthropicOptions {
                common: StreamOptions {
                    api_key: Some("controlled-key".into()),
                    fetch: Some(fetch),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
        let mut observed = Vec::new();
        let mut live = None;
        while let Some(event) = events.next().await {
            observed.push(crate::json::event_record(&event)?);
            match event {
                AssistantMessageEvent::Start { partial } => live = Some(partial),
                AssistantMessageEvent::Done { message, .. } => {
                    assert!(Arc::ptr_eq(live.as_ref().ok_or("start")?, &message));
                }
                _ => {}
            }
        }
        assert_eq!(observed, serde_json::json!([
            {"type":"start"},
            {"type":"thinking_start","contentIndex":0},
            {"type":"text_start","contentIndex":1},
            {"type":"toolcall_start","contentIndex":2},
            {"type":"thinking_delta","contentIndex":0,"delta":"plan"},
            {"type":"text_delta","contentIndex":1,"delta":"Checking"},
            {"type":"toolcall_delta","contentIndex":2,"delta":"{\"location\":"},
            {"type":"thinking_delta","contentIndex":0,"delta":" now"},
            {"type":"text_delta","contentIndex":1,"delta":" weather"},
            {"type":"toolcall_delta","contentIndex":2,"delta":"\"London\"}"},
            {"type":"thinking_end","contentIndex":0,"content":"plan now"},
            {"type":"text_end","contentIndex":1,"content":"Checking weather"},
            {"type":"toolcall_end","contentIndex":2,"toolCall":{"type":"toolCall","id":"weather-1","name":"get_weather","arguments":{"location":"London"}}},
            {"type":"done","reason":"toolUse"}
        ]).as_array().ok_or("expected events")?.as_slice());
        let final_message = events.result().await;
        assert!(Arc::ptr_eq(
            &final_message,
            live.as_ref().ok_or("live handle")?
        ));
        check_interleaved_content(&final_message)?;
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_guide_observes_failure_and_cancellation() -> TestResult {
    use maestro_models::{
        AnthropicOptions, AssistantMessageEvent, Cancellation, ErrorReason, stream_anthropic,
    };
    use serde_json::json;
    let _registry = bundled::registry();
    crate::runtime::run(async {
        check_setup_errors().await?;
        let model = crate::messages::model(&json!({}))?;
        let (mut feed, mut common) = super::fed_options();
        let signal = Cancellation::new();
        common.signal = Some(signal.clone());
        let events = stream_anthropic(
            model.clone(),
            bundled::conversation()?,
            Some(AnthropicOptions {
                common,
                ..Default::default()
            }),
        );
        let frames = [
            json!({"type":"message_start","message":{"id":"m","usage":{"input_tokens":3,"output_tokens":0}}}),
            json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
            json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Partial"}}),
        ].iter().map(crate::messages::sse).collect::<String>();
        feed.chunks.send(frames.into_bytes())?;
        let mut saw_delta = false;
        let mut terminal = None;
        while let Some(event) = events.next().await {
            if let AssistantMessageEvent::TextDelta { delta, .. } = &event {
                assert_eq!(delta, "Partial");
                feed.read_pending().await?;
                signal.abort();
                saw_delta = true;
            }
            if let AssistantMessageEvent::Error { reason, .. } = event {
                terminal = Some(reason);
            }
        }
        assert!(saw_delta);
        assert!(matches!(terminal, Some(ErrorReason::Aborted)));
        feed.released.await?;
        let partial = events.result().await;
        assert_eq!(bundled::text_of(&partial), "Partial");
        assert_eq!(bundled::stop_of(&partial), StopReason::Aborted);
        assert_eq!(
            partial
                .read()
                .map_err(|_| "message lock")?
                .usage
                .input
                .to_bits(),
            3.0_f64.to_bits()
        );
        continue_partial(model, partial).await?;
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Final indexed content agrees with the ordered event transcript.
fn check_interleaved_content(final_message: &maestro_models::SharedAssistantMessage) -> TestResult {
    use maestro_models::AssistantContent;
    let final_message = final_message.read().map_err(|_| "message lock")?;
    assert!(
        matches!(&final_message.content[0], AssistantContent::Thinking(t) if t.thinking == "plan now")
    );
    assert!(
        matches!(&final_message.content[1], AssistantContent::Text(t) if t.text == "Checking weather")
    );
    assert!(
        matches!(&final_message.content[2], AssistantContent::ToolCall(t) if t.arguments["location"] == "London")
    );
    assert_eq!(final_message.stop_reason, StopReason::ToolUse);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Immediate resolution failure is distinct from a bundled terminal error.
async fn check_setup_errors() -> TestResult {
    use maestro_models::{AnthropicOptions, AssistantMessageEvent, ErrorReason, stream_anthropic};
    use serde_json::json;
    let missing = crate::chat::model(&json!({"api":"guide-unregistered"}))?;
    let error = stream(missing, bundled::conversation()?, None)
        .err()
        .ok_or("setup failure")?;
    assert_eq!(
        error.message,
        "No API provider registered for api: guide-unregistered"
    );
    let model = crate::messages::model(&json!({}))?;
    let failed = stream_anthropic(
        model.clone(),
        bundled::conversation()?,
        Some(AnthropicOptions {
            client: Some(crate::messages::failing_client("controlled failure")),
            ..Default::default()
        }),
    );
    let failures = bundled::drain(&failed).await;
    assert!(failures.iter().any(|event| matches!(
        event,
        AssistantMessageEvent::Error {
            reason: ErrorReason::Error,
            ..
        }
    )));
    let failure = failures.last().ok_or("terminal failure")?;
    assert_eq!(serde_json::to_value(failure)?["type"], "error");
    assert_eq!(bundled::stop_of(&failed.result().await), StopReason::Error);
    assert_eq!(
        serde_json::to_value([
            StopReason::Stop,
            StopReason::Length,
            StopReason::ToolUse,
            StopReason::Error,
            StopReason::Aborted
        ])?,
        json!(["stop", "length", "toolUse", "error", "aborted"])
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Continue from the returned partial, with request projection omitting the failed attempt.
async fn continue_partial(
    model: maestro_models::Model,
    partial: maestro_models::SharedAssistantMessage,
) -> TestResult {
    use maestro_models::Message;
    use serde_json::json;
    let mut context = bundled::conversation()?;
    context.messages.push(Message::Assistant(
        partial.read().map_err(|_| "message lock")?.clone(),
    ));
    let (common, requests) = response_options(&model.api, "Continued");
    let final_message = complete(
        model,
        context,
        Some(ProviderStreamOptions {
            common,
            ..Default::default()
        }),
    )
    .await?;
    assert_eq!(bundled::text_of(&final_message), "Continued");
    assert_eq!(
        bundled::recorded(&requests)[0].body["messages"],
        json!([{"role":"user","content":[{"type":"text","text":"hi","cache_control":{"type":"ephemeral"}}]}])
    );
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Consume each streaming invocation before independently observing its result.
async fn check_quick_stream(events: maestro_models::AssistantMessageEventStream) -> TestResult {
    let mut deltas = String::new();
    while let Some(event) = events.next().await {
        if let maestro_models::AssistantMessageEvent::TextDelta { delta, .. } = event {
            deltas.push_str(&delta);
        }
    }
    assert_eq!(deltas, "Hello");
    check_quick_result(&events.result().await)?;
    Ok(())
}
