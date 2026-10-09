//! Reduction of response events into the shared assistant message.

use super::super::{OpenAIResponsesStreamOptions, process_responses_stream};
use super::{TestResult, block_on, responses};
use crate as maestro_models;
use std::sync::{Arc, Mutex};

use futures_core::Stream;
use futures_util::FutureExt;
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, DiagnosticErrorInfo, JsonObject,
    SharedAssistantMessage,
};
use responses::Script;
use serde_json::{Value, json};

/// Rows of reduction cases and the expectations they carry.
const EVENTS: &str = include_str!("fixtures/events.json");

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
    event_rows("maestro_responses_events_encode_text_signature", 3)?;
    for (identity, phase, signature) in [
        ("", "false", r#"{"v":1}"#),
        (",\"id\":null", "null", r#"{"v":1,"id":null}"#),
        (",\"id\":42", "[]", r#"{"v":1,"id":42,"phase":[]}"#),
        (",\"id\":\"m\"", "true", r#"{"v":1,"id":"m","phase":true}"#),
    ] {
        let events = vec![
            r#"{"type":"response.output_item.added","item":{"type":"message"}}"#.to_owned(),
            format!(
                r#"{{"type":"response.output_item.done","item":{{"type":"message"{identity},"phase":{phase},"content":[]}}}}"#
            ),
            r#"{"type":"response.completed"}"#.to_owned(),
        ];
        let run = block_on(false, responses::reduce(Script::of(events)?))?;
        run.outcome?;
        assert_eq!(
            serde_json::to_value(&run.message.content)?[0]["textSignature"],
            signature
        );
    }
    Ok(())
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
    event_rows("maestro_responses_events_preserve_reasoning_item_json", 2)
}

#[test]
fn maestro_responses_events_finish_argument_suffix() -> TestResult {
    event_rows("maestro_responses_events_finish_argument_suffix", 4)
}

#[test]
fn maestro_responses_events_remove_tool_scratch() -> TestResult {
    event_rows("maestro_responses_events_remove_tool_scratch", 2)
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
    event_rows("maestro_responses_events_map_completion_status", 10)?;
    for (status, error) in [
        ("false", None),
        ("null", None),
        ("0", None),
        (r#""complet\u0065d""#, None),
        ("[]", Some("Unhandled stop reason: ")),
        ("{}", Some("Unhandled stop reason: [object Object]")),
        (r#"["completed"]"#, Some("Unhandled stop reason: completed")),
        ("42", Some("Unhandled stop reason: 42")),
    ] {
        let event = format!(r#"{{"type":"response.completed","response":{{"status":{status}}}}}"#);
        let run = block_on(false, responses::reduce(Script::of(vec![event])?))?;
        assert_eq!(run.outcome.err().map(|e| e.message).as_deref(), error);
    }
    Ok(())
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
    event_rows("maestro_responses_events_render_direct_errors", 16)
}

#[test]
fn maestro_responses_events_render_failed_details() -> TestResult {
    event_rows("maestro_responses_events_render_failed_details", 24)
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
    event_rows("maestro_responses_events_reject_lone_surrogates", 8)
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
/// before the final item.
fn arguments_kept(text: &str) -> TestResult<JsonObject> {
    let added = json!({"type": "response.output_item.added", "item": {"type": "function_call",
        "id": "fc_1", "call_id": "c", "name": "lookup", "arguments": ""}});
    let delta = json!({"type": "response.function_call_arguments.delta", "delta": text});
    let done = json!({"type": "response.output_item.done", "item": {"type": "function_call",
    }});
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
        json!({"type": "response.output_item.added", "item": {"type": "message", "content": []}}),
        json!({"type": "response.output_item.done", "item": {"type": "message", "id": "msg_1",
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

#[test]
fn maestro_responses_events_ignore_malformed_unselected_fields() -> TestResult {
    event_rows(
        "maestro_responses_events_ignore_malformed_unselected_fields",
        33,
    )?;
    let events = [
        r#"{"type":"future.event","delta":"\ud800","response":{"id":"\ud800"}}"#,
        r#"{"type":"response.output_item.added","item":{"type":"future.item","name":"\ud800","content":[{"text":"\ud800"}]}}"#,
        r#"{"type":"response.created","response":{"id":"first","type":"\ud800","usage":{"input_tokens":"\ud800"}}}"#,
        r#"{"type":"response.output_item.added","item":{"type":"message","id":"\ud800","content":[{"type":"output_text","text":"\ud800"}]}}"#,
        r#"{"type":"response.content_part.added","part":{"type":"output_text","text":"\ud800"}}"#,
        r#"{"type":"response.output_item.done","item":{"type":"message","id":"m","content":[{"type":"output_text","text":"kept","refusal":"\ud800"}]}}"#,
        r#"{"type":"response.function_call_arguments.done","arguments":"\ud800"}"#,
        r#"{"type":"response.completed","response":{"id":"kept","service_tier":"\ud800"}}"#,
    ].map(str::to_owned);
    let run = block_on(false, responses::reduce(Script::of(events.to_vec())?))?;
    run.outcome?;
    assert_eq!(run.message.response_id.as_deref(), Some("kept"));
    let message = serde_json::to_value(&run.message)?;
    assert_eq!(message["content"][0]["text"], "kept");
    assert_eq!(run.events.len(), 2);
    Ok(())
}

/// Append through the acknowledged start's own partial handle before the next event.
async fn append_caller_block(queue: &AssistantMessageEventStream, output: &SharedAssistantMessage) {
    let first = queue.next().await;
    let partial = match first {
        Some(
            AssistantMessageEvent::TextStart { partial, .. }
            | AssistantMessageEvent::ThinkingStart { partial, .. }
            | AssistantMessageEvent::ToolcallStart { partial, .. },
        ) => partial,
        other => panic!("start precedes producer acknowledgement: {other:?}"),
    };
    assert!(Arc::ptr_eq(&partial, output));
    partial
        .write()
        .unwrap()
        .content
        .push(maestro_models::AssistantContent::Text(
            maestro_models::TextContent {
                text: "caller".to_owned(),
                text_signature: None,
            },
        ));
}

/// A producer whose second poll acknowledges the already-published start.
fn appending_source<'a>(
    events: &'a [String],
    output: &'a SharedAssistantMessage,
    queue: &'a AssistantMessageEventStream,
) -> impl Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin + 'a {
    Box::pin(futures_util::stream::unfold(
        0_usize,
        move |step| async move {
            if step == 1 {
                append_caller_block(queue, output).await;
            }
            events.get(step).map(|event| (Ok(event.clone()), step + 1))
        },
    ))
}

#[test]
fn maestro_responses_events_keep_started_block_identity() -> TestResult {
    for (item, delta, done, key, expected) in [
        (
            json!({"type":"message","content":[{"type":"output_text"}]}),
            json!({"type":"response.output_text.delta","delta":"provisional"}),
            json!({"type":"message","id":"final","content":[{"type":"output_text","text":"original"}]}),
            "text",
            json!("original"),
        ),
        (
            json!({"type":"reasoning","summary":[]}),
            json!({"type":"response.reasoning_text.delta","delta":"provisional"}),
            json!({"type":"reasoning","summary":[{"text":"original"}]}),
            "thinking",
            json!("original"),
        ),
        (
            json!({"type":"function_call","id":"i","call_id":"c","name":"lookup","arguments":""}),
            json!({"type":"response.function_call_arguments.delta","delta":"{\"x\":1}"}),
            json!({"type":"function_call","arguments":"{\"decoy\":2}"}),
            "arguments",
            json!({"x":1}),
        ),
    ] {
        let model = responses::model(None)?;
        let output = responses::shared_output(&model, &responses::zero_usage())?;
        let queue = AssistantMessageEventStream::new();
        let events = [
            json!({"type":"response.output_item.added","item":item}),
            delta,
            json!({"type":"response.output_item.done","item":done}),
            json!({"type":"response.completed"}),
        ]
        .map(|event| event.to_string());
        let source = appending_source(&events, &output, &queue);
        block_on(false, async {
            process_responses_stream(source, &output, &queue, &model, None).await?;
            Ok(())
        })?;
        let message = serde_json::to_value(&*output.read().unwrap())?;
        assert_eq!(message["content"][0][key], expected);
        assert_eq!(message["content"][1]["text"], "caller");
        queue.end(None);
        let mut updates = Vec::new();
        while let Some(update) = queue.next().now_or_never().flatten() {
            updates.push(serde_json::to_value(update)?);
        }
        assert_eq!(updates.len(), 2);
        for update in updates {
            assert_eq!(update["contentIndex"], 0);
        }
    }
    Ok(())
}

#[test]
fn maestro_responses_events_publish_completion_before_callbacks() -> TestResult {
    for status in [r#""completed""#, r#""unknown""#, r#""\ud800""#] {
        let model = responses::model(None)?;
        let output = responses::shared_output(&model, &responses::zero_usage())?;
        let updates = AssistantMessageEventStream::new();
        let observed = Mutex::new(Vec::new());
        let observe = |phase| {
            let message = output.read().unwrap();
            assert_eq!(message.response_id.as_deref(), Some("final"));
            assert_eq!(message.usage.input.to_bits(), 10.0_f64.to_bits());
            assert_eq!(
                message.usage.cost.input.to_bits(),
                1.999_999_999_999_999_8e-5_f64.to_bits()
            );
            observed.lock().unwrap().push(phase);
        };
        let resolve = |echoed: Option<&str>, requested: Option<&str>| {
            assert_eq!(echoed, Some("flex"));
            assert_eq!(requested, Some("priority"));
            observe("resolve");
            Some("chosen".to_owned())
        };
        let price = |usage: &mut maestro_models::Usage, tier: Option<&str>| {
            assert_eq!(tier, Some("chosen"));
            observe("price");
            usage.cost.input *= 2.0;
        };
        let options = OpenAIResponsesStreamOptions {
            service_tier: Some("priority"),
            resolve_service_tier: Some(&resolve),
            apply_service_tier_pricing: Some(&price),
        };
        let event = format!(
            r#"{{"type":"response.completed","response":{{"id":"final","status":{status},"usage":{{"input_tokens":10}},"service_tier":"flex"}}}}"#
        );
        let source = futures_util::stream::iter([Ok(event)]);
        let outcome = block_on(false, async {
            Ok(process_responses_stream(source, &output, &updates, &model, Some(&options)).await)
        })?;
        match status {
            r#""completed""# => outcome?,
            r#""unknown""# => assert_eq!(
                outcome.unwrap_err().message,
                "Unhandled stop reason: unknown"
            ),
            _ => assert_eq!(outcome.unwrap_err().name, None),
        }
        assert_eq!(*observed.lock().unwrap(), vec!["resolve", "price"]);
        assert_eq!(
            output.read().unwrap().usage.cost.input.to_bits(),
            3.999_999_999_999_999_6e-5_f64.to_bits()
        );
    }
    Ok(())
}

#[test]
fn maestro_responses_events_render_objects_without_decoding_members() -> TestResult {
    event_rows(
        "maestro_responses_events_render_objects_without_decoding_members",
        3,
    )?;
    let deep = nested(100_000);
    let events = vec![format!(
        r#"{{"type":"error","code":{{"unread":{deep}}},"message":"kept"}}"#
    )];
    let run = block_on(false, responses::reduce(Script::of(events)?))?;
    assert_eq!(
        run.outcome.unwrap_err().message,
        "Error Code [object Object]: kept"
    );
    Ok(())
}

#[test]
fn maestro_responses_events_ignore_unused_final_call_members() -> TestResult {
    event_rows(
        "maestro_responses_events_ignore_unused_final_call_members",
        2,
    )?;
    let events = [
        r#"{"type":"response.output_item.added","item":{"type":"function_call","id":"i","call_id":"c","name":"lookup","arguments":"{\"x\":1}"}}"#,
        r#"{"type":"response.output_item.done","item":{"type":"function_call","id":"\ud800","name":"\ud800","arguments":"\ud800"}}"#,
        r#"{"type":"response.completed"}"#,
    ].map(str::to_owned);
    let run = block_on(false, responses::reduce(Script::of(events.to_vec())?))?;
    run.outcome?;
    let message = serde_json::to_value(&run.message)?;
    assert_eq!(message["content"][0]["arguments"], json!({"x":1}));
    assert_eq!(run.events[1]["toolCall"], message["content"][0]);
    Ok(())
}

#[test]
fn maestro_responses_events_defer_initial_part_decoding() -> TestResult {
    let start = r#"{"type":"response.output_item.added","item":{"type":"message","content":[{"type":"\ud800"}]}}"#;
    for selected in [false, true] {
        let mut events = vec![start.to_owned()];
        if selected {
            events.push(r#"{"type":"response.output_text.delta","delta":"selected"}"#.to_owned());
        }
        events.push(r#"{"type":"response.completed"}"#.to_owned());
        let run = block_on(false, responses::reduce(Script::of(events)?))?;
        if selected {
            assert_eq!(run.outcome.unwrap_err().name, None);
        } else {
            run.outcome?;
        }
        assert_eq!(run.events.len(), 1);
        assert_eq!(serde_json::to_value(run.message)?["content"][0]["text"], "");
    }
    Ok(())
}

#[test]
fn maestro_responses_events_render_nested_error_arrays() -> TestResult {
    for depth in [127, 128, 1024] {
        let value = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        let event = format!(r#"{{"type":"error","code":{value},"message":"nested"}}"#);
        let run = block_on(false, responses::reduce(Script::of(vec![event])?))?;
        assert_eq!(run.outcome.unwrap_err().message, "Error Code : nested");
    }
    Ok(())
}

#[test]
fn maestro_responses_events_join_final_values() -> TestResult {
    for (kind, parts, wanted) in [
        (
            "message",
            json!([{"type":"output_text","text":42},{"type":"refusal","refusal":false}]),
            "42false",
        ),
        (
            "message",
            json!([{"type":"output_text"},{"type":"refusal","refusal":null}]),
            "",
        ),
        (
            "reasoning",
            json!([{"text":42},{"text":false}]),
            "42\n\nfalse",
        ),
    ] {
        let added = json!({"type":"response.output_item.added","item":{"type":kind}});
        let done = json!({"type":"response.output_item.done","item":{"type":kind,"id":"m","content":parts}});
        let completed = json!({"type":"response.completed"});
        let run = block_on(
            false,
            responses::reduce(Script::of(
                [added, done, completed].map(|e| e.to_string()).to_vec(),
            )?),
        )?;
        run.outcome?;
        let content = serde_json::to_value(&run.message.content)?;
        let key = if kind == "message" {
            "text"
        } else {
            "thinking"
        };
        assert_eq!(content[0][key], wanted);
    }
    Ok(())
}

#[test]
fn maestro_responses_events_preserve_encoded_phase_and_missing_identity() -> TestResult {
    let events = [
        r#"{"type":"response.output_item.added","item":{"type":"message"}}"#,
        r#"{"type":"response.output_item.done","item":{"type":"message","id":"m","phase":"other","content":[]}}"#,
        r#"{"type":"response.output_item.done","item":{"type":"function_call","name":"lookup"}}"#,
        r#"{"type":"response.completed"}"#,
    ];
    let run = block_on(
        false,
        responses::reduce(Script::of(events.map(str::to_owned).to_vec())?),
    )?;
    run.outcome?;
    let content = serde_json::to_value(&run.message.content)?;
    assert_eq!(
        content[0]["textSignature"],
        r#"{"v":1,"id":"m","phase":"other"}"#
    );
    assert_eq!(content[1]["id"], "undefined|undefined");
    Ok(())
}

#[test]
fn maestro_responses_events_require_selected_containers() -> TestResult {
    for event in [
        "null",
        r#"{"type":"response.created"}"#,
        r#"{"type":"response.created","response":null}"#,
        r#"{"type":"response.output_item.added"}"#,
        r#"{"type":"response.output_item.added","item":null}"#,
        r#"{"type":"response.output_item.done"}"#,
        r#"{"type":"response.output_item.done","item":null}"#,
    ] {
        let run = block_on(
            false,
            responses::reduce(Script::of(vec![
                event.to_owned(),
                r#"{"type":"response.completed"}"#.to_owned(),
            ])?),
        )?;
        assert!(run.outcome.is_err(), "{event}");
        assert!(run.events.is_empty());
    }
    for content in ["", ",\"content\":null", ",\"content\":{}"] {
        let events = vec![
            r#"{"type":"response.output_item.added","item":{"type":"message"}}"#.to_owned(),
            format!(
                r#"{{"type":"response.output_item.done","item":{{"type":"message"{content}}}}}"#
            ),
        ];
        let run = block_on(false, responses::reduce(Script::of(events)?))?;
        let error = run
            .outcome
            .err()
            .ok_or("selected content must fail before EOF")?;
        assert_ne!(
            error.message,
            "Response stream ended before a terminal event"
        );
        assert_eq!(run.events, [json!({"type":"text_start","contentIndex":0})]);
    }
    Ok(())
}

#[test]
fn maestro_responses_events_concatenate_raw_deltas() -> TestResult {
    for kind in ["message", "reasoning", "function_call"] {
        let delta = match kind {
            "message" => "response.output_text.delta",
            "reasoning" => "response.reasoning_text.delta",
            _ => "response.function_call_arguments.delta",
        };
        let mut events = vec![json!({"type":"response.output_item.added","item":{"type":kind,"name":"lookup","content":[{"type":"output_text"}]}}).to_string()];
        for value in [
            json!(42),
            json!(false),
            Value::Null,
            json!({}),
            json!([1, null, 2]),
        ] {
            events.push(json!({"type":delta,"delta":value}).to_string());
        }
        events.push(format!(r#"{{"type":"{delta}"}}"#));
        events.push(r#"{"type":"response.completed"}"#.to_owned());
        let run = block_on(false, responses::reduce(Script::of(events)?))?;
        run.outcome?;
        let deltas: Vec<&Value> = run.events.iter().filter_map(|e| e.get("delta")).collect();
        assert_eq!(
            deltas,
            vec![
                &json!("42"),
                &json!("false"),
                &json!("null"),
                &json!("[object Object]"),
                &json!("1,,2"),
                &json!("undefined")
            ]
        );
        if kind != "function_call" {
            let content = serde_json::to_value(&run.message.content)?;
            let key = if kind == "message" {
                "text"
            } else {
                "thinking"
            };
            assert_eq!(content[0][key], "42falsenull[object Object]1,,2undefined");
        }
    }
    Ok(())
}

#[test]
fn maestro_responses_events_keep_final_text_when_signature_fails() -> TestResult {
    let events = [
        r#"{"type":"response.output_item.added","item":{"type":"message","content":[{"type":"output_text"}]}}"#,
        r#"{"type":"response.output_text.delta","delta":"draft"}"#,
        r#"{"type":"response.output_item.done","item":{"type":"message","id":"\ud800","content":[{"type":"output_text","text":"final"}]}}"#,
    ];
    let run = block_on(
        false,
        responses::reduce(Script::of(events.map(str::to_owned).to_vec())?),
    )?;
    assert!(run.outcome.is_err());
    assert_eq!(
        serde_json::to_value(&run.message.content)?,
        json!([{"type":"text","text":"final"}])
    );
    assert_eq!(
        run.events,
        [
            json!({"type":"text_start","contentIndex":0}),
            json!({"type":"text_delta","contentIndex":0,"delta":"draft"})
        ]
    );
    Ok(())
}

#[test]
fn maestro_responses_events_reject_unrepresentable_assigned_strings() -> TestResult {
    for event in [
        r#"{"type":"response.output_item.added","item":{"type":"function_call"}}"#,
        r#"{"type":"response.output_item.added","item":{"type":"function_call","name":false}}"#,
        r#"{"type":"response.created","response":{"id":true}}"#,
        r#"{"type":"response.completed","response":{"id":true}}"#,
        r#"{"type":"response.output_item.added","item":{"type":"function_call","name":"lookup","arguments":[]}}"#,
    ] {
        let run = block_on(
            false,
            responses::reduce(Script::of(vec![
                event.to_owned(),
                r#"{"type":"response.completed"}"#.to_owned(),
            ])?),
        )?;
        assert!(run.outcome.is_err(), "{event}");
        assert!(run.message.content.is_empty());
        assert!(run.events.is_empty());
    }
    for value in [Value::Null, json!(false), json!(0), json!("")] {
        let run = block_on(false, responses::reduce(Script::of(vec![json!({"type":"response.output_item.added","item":{"type":"function_call","name":"lookup","arguments":value}}).to_string(),
            r#"{"type":"response.completed","response":{"id":false}}"#.to_owned()])?))?;
        run.outcome?;
        assert!(run.message.response_id.is_none());
        assert_eq!(
            serde_json::to_value(&run.message.content)?[0]["arguments"],
            json!({})
        );
    }
    Ok(())
}

#[test]
fn maestro_responses_events_validate_initial_arrays_only_when_used() -> TestResult {
    for (kind, field, selected) in [
        (
            "message",
            "content",
            r#"{"type":"response.output_text.delta","delta":"kept"}"#,
        ),
        (
            "reasoning",
            "summary",
            r#"{"type":"response.reasoning_summary_text.delta","delta":"kept"}"#,
        ),
        (
            "reasoning",
            "summary",
            r#"{"type":"response.reasoning_summary_part.done"}"#,
        ),
        (
            "reasoning",
            "summary",
            r#"{"type":"response.reasoning_summary_part.added","part":{}}"#,
        ),
    ] {
        for container in [r#""\ud800""#, r#"{"length":1,"0":{"type":"output_text"}}"#] {
            for use_container in [false, true] {
                let mut events = vec![format!(
                    r#"{{"type":"response.output_item.added","item":{{"type":"{kind}","{field}":{container}}}}}"#
                )];
                events.extend(use_container.then(|| selected.to_owned()));
                events.push(r#"{"type":"response.completed"}"#.to_owned());
                let run = block_on(false, responses::reduce(Script::of(events)?))?;
                assert_eq!(run.outcome.is_err(), use_container);
                assert_eq!(run.events.len(), 1);
                assert_eq!(
                    run.events[0]["type"],
                    format!(
                        "{}_start",
                        if kind == "message" {
                            "text"
                        } else {
                            "thinking"
                        }
                    )
                );
            }
        }
    }
    Ok(())
}

#[test]
fn maestro_responses_events_reject_selected_tier_and_argument_types() -> TestResult {
    for pricing in [false, true] {
        let mut script = Script::of(vec![
            r#"{"type":"response.completed","response":{"service_tier":42}}"#.to_owned(),
        ])?;
        script.options = json!({"pricing":pricing});
        let run = block_on(false, responses::reduce(script))?;
        assert_eq!(run.outcome.is_err(), pricing);
        assert!(run.tiers.is_empty());
    }
    for arguments in ["null", "true", "42", "[]", "{}"] {
        let events = vec![r#"{"type":"response.output_item.added","item":{"type":"function_call","name":"lookup"}}"#.to_owned(),
            r#"{"type":"response.function_call_arguments.delta","delta":"{\"x\":1}"}"#.to_owned(),
            format!(r#"{{"type":"response.function_call_arguments.done","arguments":{arguments}}}"#)];
        let run = block_on(false, responses::reduce(Script::of(events)?))?;
        let error = run
            .outcome
            .err()
            .ok_or("selected argument assignment fails")?;
        assert_ne!(
            error.message,
            "Response stream ended before a terminal event"
        );
        assert_eq!(
            serde_json::to_value(&run.message.content)?[0]["arguments"],
            json!({"x":1})
        );
        assert_eq!(
            run.events,
            [
                json!({"type":"toolcall_start","contentIndex":0}),
                json!({"type":"toolcall_delta","contentIndex":0,"delta":"{\"x\":1}"})
            ]
        );
    }
    Ok(())
}
