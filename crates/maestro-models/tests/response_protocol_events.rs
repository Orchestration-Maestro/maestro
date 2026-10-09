//! Reduction of response events into the shared assistant message.

#[allow(
    dead_code,
    reason = "Each test binary uses part of the shared helpers."
)]
#[path = "support/chat.rs"]
mod chat;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the shared helpers."
)]
#[path = "support/responses.rs"]
mod responses;

use std::sync::{Arc, Mutex};

use chat::{TestResult, block_on};
use futures_core::Stream;
use futures_util::FutureExt;
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, DiagnosticErrorInfo, JsonObject,
    SharedAssistantMessage, process_responses_stream,
};
use responses::Script;
use serde_json::{Value, json};

/// Rows of reduction cases and the expectations they carry.
const EVENTS: &str = include_str!("fixtures/responses/events.json");

/// Reduce every event row a test owns.
fn event_rows(test: &str, count: usize) -> TestResult {
    block_on(false, async {
        for row in responses::rows(EVENTS, test, count)? {
            responses::check_events(row).await?;
        }
        Ok(())
    })
}

#[test]
fn maestro_responses_events_retain_response_identity() -> TestResult {
    event_rows("maestro_responses_events_retain_response_identity", 2)
}

#[test]
fn maestro_responses_events_replace_final_text() -> TestResult {
    event_rows("maestro_responses_events_replace_final_text", 1)
}

#[test]
fn maestro_responses_events_encode_text_signature() -> TestResult {
    event_rows("maestro_responses_events_encode_text_signature", 3)
}

#[test]
fn maestro_responses_events_accept_refusal_parts() -> TestResult {
    event_rows("maestro_responses_events_accept_refusal_parts", 2)
}

#[test]
fn maestro_responses_events_gate_deltas_by_current_part() -> TestResult {
    event_rows("maestro_responses_events_gate_deltas_by_current_part", 37)
}

#[test]
fn maestro_responses_events_accumulate_reasoning_parts() -> TestResult {
    event_rows("maestro_responses_events_accumulate_reasoning_parts", 1)
}

#[test]
fn maestro_responses_events_choose_final_reasoning_text() -> TestResult {
    event_rows("maestro_responses_events_choose_final_reasoning_text", 4)
}

#[test]
fn maestro_responses_events_preserve_reasoning_item_json() -> TestResult {
    event_rows("maestro_responses_events_preserve_reasoning_item_json", 1)
}

#[test]
fn maestro_responses_events_finish_argument_suffix() -> TestResult {
    event_rows("maestro_responses_events_finish_argument_suffix", 4)
}

#[test]
fn maestro_responses_events_remove_tool_scratch() -> TestResult {
    event_rows("maestro_responses_events_remove_tool_scratch", 1)
}

#[test]
fn maestro_responses_events_fallback_to_final_arguments() -> TestResult {
    event_rows("maestro_responses_events_fallback_to_final_arguments", 4)
}

#[test]
fn maestro_responses_events_insert_final_only_tool() -> TestResult {
    event_rows("maestro_responses_events_insert_final_only_tool", 1)
}

#[test]
fn maestro_responses_events_map_completion_status() -> TestResult {
    event_rows("maestro_responses_events_map_completion_status", 9)
}

#[test]
fn maestro_responses_events_preserve_tool_finish_priority() -> TestResult {
    event_rows("maestro_responses_events_preserve_tool_finish_priority", 3)
}

#[test]
fn maestro_responses_events_account_usage_categories() -> TestResult {
    event_rows("maestro_responses_events_account_usage_categories", 4)
}

#[test]
fn maestro_responses_events_keep_usage_when_absent() -> TestResult {
    event_rows("maestro_responses_events_keep_usage_when_absent", 1)
}

#[test]
fn maestro_responses_events_choose_service_tier() -> TestResult {
    event_rows("maestro_responses_events_choose_service_tier", 10)
}

#[test]
fn maestro_responses_events_use_tier_resolver() -> TestResult {
    event_rows("maestro_responses_events_use_tier_resolver", 2)
}

#[test]
fn maestro_responses_events_skip_resolver_without_pricing() -> TestResult {
    event_rows("maestro_responses_events_skip_resolver_without_pricing", 1)
}

#[test]
fn maestro_responses_events_render_direct_errors() -> TestResult {
    event_rows("maestro_responses_events_render_direct_errors", 5)
}

#[test]
fn maestro_responses_events_render_failed_details() -> TestResult {
    event_rows("maestro_responses_events_render_failed_details", 7)
}

#[test]
fn maestro_responses_events_reject_incomplete_eof() -> TestResult {
    event_rows("maestro_responses_events_reject_incomplete_eof", 3)
}

#[test]
fn maestro_responses_events_accept_incomplete_terminal() -> TestResult {
    event_rows("maestro_responses_events_accept_incomplete_terminal", 1)
}

#[test]
fn maestro_responses_events_ignore_unknown_events() -> TestResult {
    event_rows("maestro_responses_events_ignore_unknown_events", 1)
}

#[test]
fn maestro_responses_events_keep_current_item_order() -> TestResult {
    event_rows("maestro_responses_events_keep_current_item_order", 2)
}

#[test]
fn maestro_responses_events_read_numeric_usage() -> TestResult {
    event_rows("maestro_responses_events_read_numeric_usage", 2)
}

#[test]
fn maestro_responses_events_read_last_duplicate_fields() -> TestResult {
    event_rows("maestro_responses_events_read_last_duplicate_fields", 1)
}

#[test]
fn maestro_responses_events_require_object_arguments() -> TestResult {
    event_rows("maestro_responses_events_require_object_arguments", 6)
}

#[test]
fn maestro_responses_events_reject_malformed_json() -> TestResult {
    event_rows("maestro_responses_events_reject_malformed_json", 1)
}

#[test]
fn maestro_responses_events_reject_lone_surrogates() -> TestResult {
    event_rows("maestro_responses_events_reject_lone_surrogates", 1)
}

#[test]
fn maestro_responses_events_propagate_source_failure() -> TestResult {
    event_rows("maestro_responses_events_propagate_source_failure", 1)
}

/// `containers` arrays nested around `0`.
fn nested(containers: usize) -> String {
    format!("{}0{}", "[".repeat(containers), "]".repeat(containers))
}

#[test]
fn maestro_responses_events_skip_unread_deep_values() -> TestResult {
    let deep = nested(100_000);
    let events = vec![
        format!(r#"{{"type":"response.created","ignored":{deep},"response":{{"id":"first"}}}}"#),
        format!(
            r#"{{"type":"response.output_item.added","item":{{"type":"message","id":"m","ignored":{deep}}}}}"#
        ),
        format!(
            r#"{{"type":"response.output_item.done","item":{{"type":"message","id":"m","ignored":{deep},"content":[{{"type":"output_text","text":"kept","ignored":{deep}}}]}}}}"#
        ),
        format!(
            r#"{{"type":"response.completed","response":{{"id":"done","status":"completed","output":{deep},"usage":{{"input_tokens":1,"ignored":{deep}}}}}}}"#
        ),
    ];
    let run = block_on(false, responses::reduce(Script::of(events)?))?;
    run.outcome?;
    assert_eq!(run.message.response_id.as_deref(), Some("done"));
    let message = serde_json::to_value(&run.message)?;
    assert_eq!(message["usage"]["input"], json!(1.0));
    assert_eq!(message["content"][0]["text"], "kept");
    Ok(())
}

/// The arguments the reducer keeps for a call whose arguments arrive as `text`, streamed
/// and then repeated by the final item.
fn arguments_kept(text: &str) -> TestResult<JsonObject> {
    let added = json!({"type": "response.output_item.added", "item": {"type": "function_call",
        "id": "fc_1", "call_id": "c", "name": "lookup", "arguments": ""}});
    let delta = json!({"type": "response.function_call_arguments.delta", "delta": text});
    let done = json!({"type": "response.output_item.done", "item": {"type": "function_call",
        "id": "fc_1", "call_id": "c", "name": "lookup", "arguments": text}});
    let completed = json!({"type": "response.completed", "response": {"status": "completed"}});
    let events = [added, delta, done, completed].map(|event| event.to_string());
    let run = block_on(false, responses::reduce(Script::of(events.to_vec())?))?;
    run.outcome?;
    match run.message.content.first() {
        Some(maestro_models::AssistantContent::ToolCall(call)) => Ok(call.arguments.clone()),
        other => Err(format!("expected a tool call, got {other:?}").into()),
    }
}

#[test]
fn maestro_responses_events_preserve_argument_boundaries() -> TestResult {
    for depth in [127_usize, 128, 100_000] {
        for closed in [true, false] {
            let inner = "[".repeat(depth - 1);
            let text = if closed {
                format!(r#"{{"x":{inner}0{}}}"#, "]".repeat(depth - 1))
            } else {
                format!(r#"{{"x":{inner}0"#)
            };
            let arguments = arguments_kept(&text)?;
            if depth > 127 {
                assert!(arguments.is_empty(), "{depth} containers, closed {closed}");
                continue;
            }
            let (mut containers, mut child) = (1, arguments.get("x"));
            while let Some(Value::Array(items)) = child {
                containers += 1;
                child = items.first();
            }
            assert_eq!(
                (containers, child),
                (depth, Some(&json!(0))),
                "closed {closed}"
            );
        }
    }
    for (text, expected) in [
        (
            r#"{"n":9007199254740993}"#,
            json!(9_007_199_254_740_992_u64),
        ),
        (r#"{"n":9007199254740993"#, json!(9_007_199_254_740_992_u64)),
        (r#"{"n":1e400}"#, Value::Null),
        (r#"{"n":1e400"#, Value::Null),
        (r#"{"n":0.99999999999999999}"#, json!(1)),
    ] {
        let arguments = arguments_kept(text)?;
        assert_eq!(arguments["n"].as_f64(), expected.as_f64(), "{text}");
        assert_eq!(arguments["n"].is_null(), expected.is_null(), "{text}");
    }
    Ok(())
}

/// The first update a reduction published, kept to inspect after the reduction ended.
type KeptUpdate = Arc<Mutex<Option<SharedAssistantMessage>>>;

/// Take the first update off the stream while the source is paused, and check that it shares
/// the watched message, which already holds the block the first event opened.
///
/// The reducer publishes before it polls the source again, so the update is already queued.
fn keep_first_update(
    watched: &SharedAssistantMessage,
    queue: &AssistantMessageEventStream,
    kept: &KeptUpdate,
) -> TestResult {
    let first = queue.next().now_or_never().flatten();
    let Some(AssistantMessageEvent::TextStart { partial, .. }) = first else {
        return Err("the first event published its text start".into());
    };
    assert!(Arc::ptr_eq(&partial, watched));
    let blocks = watched.read().map(|message| message.content.len());
    assert_eq!(blocks.ok(), Some(1), "the block is visible while reducing");
    *kept.lock().map_err(|error| error.to_string())? = Some(partial);
    Ok(())
}

/// A source that yields `events` and, before its second event, keeps the first update.
///
/// It is polled for its next event only after the reducer finished the previous one, so what
/// it observes at the second poll is the reduction of the first event.
fn watching_source(
    events: [String; 3],
    output: &SharedAssistantMessage,
    stream: &AssistantMessageEventStream,
    kept: &KeptUpdate,
) -> impl Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin {
    let (watched, queue, kept) = (Arc::clone(output), stream.clone(), Arc::clone(kept));
    Box::pin(futures_util::stream::unfold(0_usize, move |step| {
        let (watched, queue, kept) = (Arc::clone(&watched), queue.clone(), Arc::clone(&kept));
        let events = events.clone();
        async move {
            if step == 1
                && let Err(error) = keep_first_update(&watched, &queue, &kept)
            {
                let failure = DiagnosticErrorInfo {
                    name: None,
                    message: error.to_string(),
                    stack: None,
                    code: None,
                };
                return Some((Err(failure), step + 1));
            }
            events.get(step).map(|event| (Ok(event.clone()), step + 1))
        }
    }))
}

#[test]
fn maestro_responses_events_share_live_output() -> TestResult {
    let events = [
        json!({"type": "response.output_item.added", "item": {"type": "message", "id": "msg_1",
            "role": "assistant", "status": "completed", "content": []}}),
        json!({"type": "response.output_item.done", "item": {"type": "message", "id": "msg_1",
            "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": "final"}]}}),
        json!({"type": "response.completed", "response": {"id": "done", "status": "completed"}}),
    ]
    .map(|event| event.to_string());
    let model = responses::model(None)?;
    let output = responses::shared_output(&model, &responses::zero_usage())?;
    let stream = AssistantMessageEventStream::new();
    let kept = KeptUpdate::default();
    let source = watching_source(events, &output, &stream, &kept);
    block_on(false, async {
        process_responses_stream(source, &output, &stream, &model, None).await?;
        Ok(())
    })?;

    stream.end(None);
    let retained = kept.lock().map_err(|error| error.to_string())?.take();
    let retained = retained.ok_or("the first update was observed")?;
    assert!(Arc::ptr_eq(&retained, &output));
    let seen = serde_json::to_value(&*retained.read().map_err(|error| error.to_string())?)?;
    assert_eq!(
        seen["content"][0]["text"], "final",
        "the handle shows later reductions"
    );
    let tail = block_on(false, async { Ok(stream.next().await) })?;
    let Some(AssistantMessageEvent::TextEnd {
        partial, content, ..
    }) = tail
    else {
        return Err(format!("expected the text end, got {tail:?}").into());
    };
    assert!(Arc::ptr_eq(&partial, &output));
    assert_eq!(content, "final");
    Ok(())
}
