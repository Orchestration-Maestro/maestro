//! Reduction of content-generation response items through the public processor.

#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::json;

#[cfg(not(target_arch = "wasm32"))]
use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::future::Future;
#[cfg(not(target_arch = "wasm32"))]
use std::pin::{Pin, pin};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use std::task::{Context, Poll, Waker};

#[cfg(not(target_arch = "wasm32"))]
use chat::{TestResult, model};
#[cfg(not(target_arch = "wasm32"))]
use json::{canonical, event_record};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::content::google_shared::process_google_stream;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, DiagnosticCode, DiagnosticErrorInfo,
    DoneReason, SharedAssistantMessage,
};
#[cfg(not(target_arch = "wasm32"))]
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
/// Every test name a fixture row may select.
const TEST_NAMES: [&str; 36] = [
    "empty_and_metadata_only_streams_leave_the_message_unchanged",
    "first_nonempty_response_identity_survives_later_chunks",
    "empty_response_identity_can_be_replaced_by_absence",
    "first_candidate_wins_without_reading_later_content",
    "text_deltas_join_without_trimming_or_deduplication",
    "thinking_switches_close_the_previous_block_in_order",
    "signature_alone_never_makes_a_thinking_block",
    "last_nonempty_signature_survives_missing_and_empty_deltas",
    "nontext_parts_do_not_change_the_current_block",
    "one_part_can_emit_text_before_its_function_call",
    "tool_calls_close_text_or_thinking_before_three_tool_events",
    "tool_calls_keep_server_ids_and_distinct_arguments",
    "missing_empty_and_duplicate_ids_get_unique_generated_ids",
    "generated_call_names_preserve_missing_and_empty_distinctions",
    "tool_signatures_are_retained_only_when_nonempty",
    "tool_arguments_keep_objects_and_compact_numeric_key_order",
    "duplicate_argument_keys_keep_the_last_value_and_first_position",
    "finish_reason_maps_every_declared_outcome",
    "tool_call_keeps_length_and_error_finish_reasons",
    "tool_without_finish_reason_keeps_the_previous_stop_outcome",
    "later_finish_reason_replaces_the_previous_one",
    "empty_finish_reason_does_not_change_the_outcome",
    "unknown_finish_preserves_content_but_skips_usage_and_eof_close",
    "unknown_finish_still_fails_after_a_tool_call",
    "usage_replaces_reports_and_keeps_the_reported_total",
    "usage_defaults_missing_counts_without_clamping_or_summing_total",
    "absent_or_null_usage_keeps_the_previous_report",
    "pricing_uses_each_model_rate_with_source_expression_order",
    "usage_updates_without_candidates_and_after_selected_parts",
    "clean_eof_closes_only_the_active_text_or_thinking_block",
    "source_failure_keeps_partial_content_without_closing_it",
    "malformed_json_fails_after_prior_chunks_without_eof_close",
    "later_invalid_part_keeps_earlier_updates_and_response_id",
    "unknown_finish_wins_over_later_invalid_usage",
    "unknown_members_and_unselected_deep_content_are_not_decoded",
    "duplicate_selected_fields_keep_the_last_member",
];

#[cfg(not(target_arch = "wasm32"))]
/// Rates the model charges per million tokens.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Rates {
    /// Input rate.
    input: f64,
    /// Output rate.
    output: f64,
    /// Cache read rate.
    cache_read: f64,
    /// Cache write rate.
    cache_write: f64,
}

#[cfg(not(target_arch = "wasm32"))]
/// What the processor is given.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Framed response items, in order.
    chunks: Vec<String>,
    /// Message of a failure that ends the source after the items.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_error: Option<String>,
    /// Rates that replace the default ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cost: Option<Rates>,
}

#[cfg(not(target_arch = "wasm32"))]
/// The selected fields of the final message.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Message {
    /// Content blocks.
    content: Vec<Value>,
    /// Stop reason.
    stop_reason: Value,
    /// Reported usage and cost.
    usage: Value,
    /// Response identifier.
    #[serde(default)]
    response_id: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
/// What the processor must leave behind.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Final message fields.
    message: Message,
    /// Ordered content events without their message handles.
    events: Vec<Value>,
    /// Failure message, or the placeholder for a native diagnostic.
    #[serde(default)]
    error: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
/// One recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Test that owns the case.
    test: String,
    /// Processor input.
    input: Input,
    /// Recorded result.
    expected: Expected,
}

#[cfg(not(target_arch = "wasm32"))]
/// The rows owned by `test`, after checking the whole fixture.
fn rows(test: &str) -> TestResult<Vec<Row>> {
    let all: Vec<Row> = serde_json::from_str(include_str!("fixtures/content_reduction.json"))?;
    let mut inputs = HashSet::new();
    for row in &all {
        assert!(
            TEST_NAMES.contains(&row.test.as_str()),
            "row owned by an unknown test"
        );
        assert!(
            inputs.insert(serde_json::to_string(&row.input)?),
            "duplicate input in {}",
            row.test
        );
    }
    let owned: Vec<Row> = all.into_iter().filter(|row| row.test == test).collect();
    assert!(!owned.is_empty(), "no rows for {test}");
    Ok(owned)
}

#[cfg(not(target_arch = "wasm32"))]
/// A failure with only a message.
fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: message.to_owned(),
        stack: None,
        code: None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// The message an invocation fills in.
fn initial_message() -> TestResult<SharedAssistantMessage> {
    Ok(Arc::new(RwLock::new(serde_json::from_value(json!({
        "role": "assistant", "content": [], "api": "google-generative-ai",
        "provider": "google", "model": "model",
        "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
        "stopReason": "stop", "timestamp": 1
    }))?)))
}

#[cfg(not(target_arch = "wasm32"))]
/// A model that charges `rates`, or the default rates.
fn priced(rates: Option<&Rates>) -> TestResult<maestro_models::Model> {
    let rates = rates.cloned().unwrap_or(Rates {
        input: 1.0,
        output: 2.0,
        cache_read: 3.0,
        cache_write: 4.0,
    });
    model(&json!({"api": "google-generative-ai", "provider": "google", "cost": rates}))
}

#[cfg(not(target_arch = "wasm32"))]
/// What one invocation left behind.
struct Observed {
    /// Selected fields of the final message.
    message: Value,
    /// Ordered events without their message handles.
    events: Vec<Value>,
    /// The failure the processor returned.
    error: Option<DiagnosticErrorInfo>,
}

#[cfg(not(target_arch = "wasm32"))]
/// Run the processor over `input` with `counter`.
async fn reduce(input: &Input, counter: &AtomicU64) -> TestResult<Observed> {
    let output = initial_message()?;
    let events = AssistantMessageEventStream::new();
    let items = input
        .chunks
        .iter()
        .cloned()
        .map(Ok)
        .chain(input.source_error.as_deref().map(|text| Err(failure(text))))
        .collect::<Vec<_>>();
    let result = process_google_stream(
        futures_util::stream::iter(items),
        &output,
        &events,
        &priced(input.cost.as_ref())?,
        counter,
    )
    .await;
    events.end(None);
    let mut records = Vec::new();
    while let Some(event) = events.next().await {
        records.push(event_record(&event)?);
    }
    let message = serde_json::to_value(&*output.read().map_err(|_| "message poisoned")?)?;
    let mut selected = json!({
        "content": message["content"],
        "stopReason": message["stopReason"],
        "usage": message["usage"],
    });
    if let Some(id) = message.get("responseId") {
        selected["responseId"] = id.clone();
    }
    Ok(Observed {
        message: selected,
        events: records,
        error: result.err(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Check a generated call identifier and replace it with the placeholder the fixture uses.
///
/// A placeholder reads `@generated:<name part>:<counter>`; `seen` keeps the identifier each
/// placeholder stood for, so every occurrence must be the same string.
fn normalize_ids(
    expected: &Value,
    actual: &mut Value,
    seen: &mut HashMap<String, String>,
) -> TestResult {
    match (expected, actual) {
        (Value::String(want), Value::String(got)) if want.starts_with("@generated:") => {
            if let Some(first) = seen.get(want) {
                assert_eq!(first, got, "identity of {want}");
            } else {
                let (prefix, counter) = want["@generated:".len()..]
                    .rsplit_once(':')
                    .ok_or("placeholder")?;
                let rest = got
                    .strip_prefix(&format!("{prefix}_"))
                    .ok_or_else(|| format!("name part of {want}: {got}"))?;
                let (millis, got_counter) = rest.rsplit_once('_').ok_or("counter field")?;
                assert_eq!(got_counter, counter, "counter of {want}");
                let millis: u128 = millis.parse()?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis();
                assert!(millis.abs_diff(now) < 600_000, "clock field of {want}");
                seen.insert(want.clone(), got.clone());
            }
            got.clone_from(want);
        }
        (Value::Object(want), Value::Object(got)) => {
            for (key, want) in want {
                if let Some(got) = got.get_mut(key) {
                    normalize_ids(want, got, seen)?;
                }
            }
        }
        (Value::Array(want), Value::Array(got)) => {
            for (want, got) in want.iter().zip(got) {
                normalize_ids(want, got, seen)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Compare one row's complete observation with its recorded result.
fn assert_row(row: &Row, observed: Observed) -> TestResult {
    let expected = &row.expected;
    let mut message = observed.message;
    let mut want = json!({
        "content": expected.message.content,
        "stopReason": expected.message.stop_reason,
        "usage": expected.message.usage,
    });
    if let Some(id) = &expected.message.response_id {
        want["responseId"] = json!(id);
    }
    let mut seen = HashMap::new();
    normalize_ids(&want, &mut message, &mut seen)?;
    let want_events = Value::Array(expected.events.clone());
    let mut events = Value::Array(observed.events);
    normalize_ids(&want_events, &mut events, &mut seen)?;
    assert_eq!(canonical(message), canonical(want), "{} message", row.test);
    assert_eq!(
        canonical(events),
        canonical(want_events),
        "{} events",
        row.test
    );
    match (&expected.error, observed.error) {
        (None, None) => {}
        (Some(want), Some(got)) if want == "@native-error" => {
            assert!(!got.message.is_empty(), "{} native error", row.test);
        }
        (Some(want), Some(got)) => assert_eq!(&got.message, want, "{} error", row.test),
        (want, got) => return Err(format!("{}: expected {want:?}, got {got:?}", row.test).into()),
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Run every fixture row owned by `test`, each with a fresh counter.
fn check(test: &str) -> TestResult {
    for row in rows(test)? {
        let observed = crate::runtime::run(reduce(&row.input, &AtomicU64::new(0)))?;
        assert_row(&row, observed)?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Collect the events of a finished invocation.
async fn drain(events: &AssistantMessageEventStream) -> TestResult<Vec<AssistantMessageEvent>> {
    let mut all = Vec::new();
    while let Some(event) = events.next().await {
        all.push(event);
    }
    Ok(all)
}

#[cfg(not(target_arch = "wasm32"))]
/// A one-chunk invocation with a generated call and the given counter.
fn generated_suffix(counter: &AtomicU64) -> TestResult<String> {
    let input = Input {
        chunks: vec![
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"f"}}]}}]}"#.to_owned(),
        ],
        source_error: None,
        cost: None,
    };
    let observed = crate::runtime::run(reduce(&input, counter))?;
    let id = observed.message["content"][0]["id"]
        .as_str()
        .ok_or("generated id")?
        .to_owned();
    assert!(id.starts_with("f_"));
    Ok(id.rsplit('_').next().ok_or("suffix")?.to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_and_metadata_only_streams_leave_the_message_unchanged() -> TestResult {
    check("empty_and_metadata_only_streams_leave_the_message_unchanged")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn first_nonempty_response_identity_survives_later_chunks() -> TestResult {
    check("first_nonempty_response_identity_survives_later_chunks")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_response_identity_can_be_replaced_by_absence() -> TestResult {
    check("empty_response_identity_can_be_replaced_by_absence")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn first_candidate_wins_without_reading_later_content() -> TestResult {
    check("first_candidate_wins_without_reading_later_content")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn text_deltas_join_without_trimming_or_deduplication() -> TestResult {
    check("text_deltas_join_without_trimming_or_deduplication")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn thinking_switches_close_the_previous_block_in_order() -> TestResult {
    check("thinking_switches_close_the_previous_block_in_order")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn signature_alone_never_makes_a_thinking_block() -> TestResult {
    check("signature_alone_never_makes_a_thinking_block")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn last_nonempty_signature_survives_missing_and_empty_deltas() -> TestResult {
    check("last_nonempty_signature_survives_missing_and_empty_deltas")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn nontext_parts_do_not_change_the_current_block() -> TestResult {
    check("nontext_parts_do_not_change_the_current_block")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn one_part_can_emit_text_before_its_function_call() -> TestResult {
    check("one_part_can_emit_text_before_its_function_call")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_calls_close_text_or_thinking_before_three_tool_events() -> TestResult {
    check("tool_calls_close_text_or_thinking_before_three_tool_events")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_calls_keep_server_ids_and_distinct_arguments() -> TestResult {
    check("tool_calls_keep_server_ids_and_distinct_arguments")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn missing_empty_and_duplicate_ids_get_unique_generated_ids() -> TestResult {
    check("missing_empty_and_duplicate_ids_get_unique_generated_ids")?;
    let shared = AtomicU64::new(0);
    assert_eq!(generated_suffix(&shared)?, "1");
    assert_eq!(generated_suffix(&shared)?, "2");
    let (first, second) = (AtomicU64::new(0), AtomicU64::new(0));
    let interleaved = [&first, &second, &first, &second]
        .into_iter()
        .map(generated_suffix)
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(interleaved, ["1", "1", "2", "2"]);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generated_call_names_preserve_missing_and_empty_distinctions() -> TestResult {
    check("generated_call_names_preserve_missing_and_empty_distinctions")?;
    let input = Input {
        chunks: vec![
            r#"{"candidates":[{"content":{"parts":[{"functionCall":{}},{"functionCall":{"name":""}}]}}]}"#
                .to_owned(),
        ],
        source_error: None,
        cost: None,
    };
    let observed = crate::runtime::run(reduce(&input, &AtomicU64::new(0)))?;
    let content = observed.message["content"].as_array().ok_or("content")?;
    let names: Vec<_> = content.iter().map(|call| call["name"].as_str()).collect();
    assert_eq!(names, [Some(""), Some("")]);
    let ids: Vec<_> = content.iter().map(|call| call["id"].as_str()).collect();
    assert!(ids[0].ok_or("id")?.starts_with("undefined_"));
    let empty = ids[1].ok_or("id")?;
    assert!(empty.starts_with('_') && empty.ends_with("_2"));
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_signatures_are_retained_only_when_nonempty() -> TestResult {
    check("tool_signatures_are_retained_only_when_nonempty")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_arguments_keep_objects_and_compact_numeric_key_order() -> TestResult {
    check("tool_arguments_keep_objects_and_compact_numeric_key_order")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn duplicate_argument_keys_keep_the_last_value_and_first_position() -> TestResult {
    check("duplicate_argument_keys_keep_the_last_value_and_first_position")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn finish_reason_maps_every_declared_outcome() -> TestResult {
    check("finish_reason_maps_every_declared_outcome")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_call_keeps_length_and_error_finish_reasons() -> TestResult {
    check("tool_call_keeps_length_and_error_finish_reasons")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_without_finish_reason_keeps_the_previous_stop_outcome() -> TestResult {
    check("tool_without_finish_reason_keeps_the_previous_stop_outcome")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn later_finish_reason_replaces_the_previous_one() -> TestResult {
    check("later_finish_reason_replaces_the_previous_one")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_finish_reason_does_not_change_the_outcome() -> TestResult {
    check("empty_finish_reason_does_not_change_the_outcome")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_finish_preserves_content_but_skips_usage_and_eof_close() -> TestResult {
    check("unknown_finish_preserves_content_but_skips_usage_and_eof_close")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_finish_still_fails_after_a_tool_call() -> TestResult {
    check("unknown_finish_still_fails_after_a_tool_call")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn usage_replaces_reports_and_keeps_the_reported_total() -> TestResult {
    check("usage_replaces_reports_and_keeps_the_reported_total")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn usage_defaults_missing_counts_without_clamping_or_summing_total() -> TestResult {
    check("usage_defaults_missing_counts_without_clamping_or_summing_total")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn absent_or_null_usage_keeps_the_previous_report() -> TestResult {
    check("absent_or_null_usage_keeps_the_previous_report")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pricing_uses_each_model_rate_with_source_expression_order() -> TestResult {
    check("pricing_uses_each_model_rate_with_source_expression_order")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn usage_updates_without_candidates_and_after_selected_parts() -> TestResult {
    check("usage_updates_without_candidates_and_after_selected_parts")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn clean_eof_closes_only_the_active_text_or_thinking_block() -> TestResult {
    check("clean_eof_closes_only_the_active_text_or_thinking_block")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn source_failure_keeps_partial_content_without_closing_it() -> TestResult {
    check("source_failure_keeps_partial_content_without_closing_it")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn malformed_json_fails_after_prior_chunks_without_eof_close() -> TestResult {
    check("malformed_json_fails_after_prior_chunks_without_eof_close")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn later_invalid_part_keeps_earlier_updates_and_response_id() -> TestResult {
    check("later_invalid_part_keeps_earlier_updates_and_response_id")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_finish_wins_over_later_invalid_usage() -> TestResult {
    check("unknown_finish_wins_over_later_invalid_usage")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_members_and_unselected_deep_content_are_not_decoded() -> TestResult {
    check("unknown_members_and_unselected_deep_content_are_not_decoded")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn duplicate_selected_fields_keep_the_last_member() -> TestResult {
    check("duplicate_selected_fields_keep_the_last_member")
}

#[cfg(not(target_arch = "wasm32"))]
/// A call whose only argument holds `depth` nested arrays.
fn deep_call(depth: usize) -> String {
    let value = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    format!(
        r#"{{"candidates":[{{"content":{{"parts":[{{"text":"kept"}},{{"functionCall":{{"id":"c","name":"n","args":{{"value":{value}}}}}}}]}}}}]}}"#
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selected_tool_arguments_enforce_the_existing_json_depth_bound() -> TestResult {
    for (depth, accepted) in [(126, true), (127, false), (128, false)] {
        let input = Input {
            chunks: vec![deep_call(depth)],
            source_error: None,
            cost: None,
        };
        let observed = crate::runtime::run(reduce(&input, &AtomicU64::new(0)))?;
        let kinds: Vec<_> = observed.events.iter().map(|e| e["type"].clone()).collect();
        if accepted {
            assert!(observed.error.is_none());
            assert_eq!(kinds.last(), Some(&json!("toolcall_end")));
        } else {
            let error = observed.error.ok_or("depth failure")?;
            assert!(!error.message.is_empty());
            assert_eq!(
                kinds,
                [json!("text_start"), json!("text_delta"), json!("text_end")]
            );
            assert_eq!(
                observed.message["content"].as_array().map(Vec::len),
                Some(1)
            );
        }
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generated_identifier_counter_advances_before_arguments_fail() -> TestResult {
    let deep = deep_call(127).replace(r#""id":"c","#, "");
    let valid = r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"n"}}]}}]}"#;
    let counter = AtomicU64::new(0);
    for (chunk, fails) in [(deep, true), (valid.to_owned(), false)] {
        let input = Input {
            chunks: vec![chunk],
            source_error: None,
            cost: None,
        };
        let observed = crate::runtime::run(reduce(&input, &counter))?;
        assert_eq!(observed.error.is_some(), fails);
    }
    assert_eq!(counter.load(Ordering::Relaxed), 2);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// A source fed through a channel, so a test controls when each item arrives.
struct Gate(tokio::sync::mpsc::UnboundedReceiver<Result<String, DiagnosticErrorInfo>>);

#[cfg(not(target_arch = "wasm32"))]
impl futures_core::Stream for Gate {
    type Item = Result<String, DiagnosticErrorInfo>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_recv(cx)
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// The partial message an event carries, when it has one.
fn partial(event: &AssistantMessageEvent) -> Option<&SharedAssistantMessage> {
    match event {
        AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. } => Some(partial),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// The text of the first content block.
fn first_text(message: &SharedAssistantMessage) -> TestResult<String> {
    let value = serde_json::to_value(&*message.read().map_err(|_| "poisoned")?)?;
    Ok(value["content"][0]["text"]
        .as_str()
        .ok_or("text")?
        .to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn partial_handles_observe_later_updates_without_retaining_the_owner() -> TestResult {
    crate::runtime::run(async {
        let output = initial_message()?;
        let weak = Arc::downgrade(&output);
        let events = AssistantMessageEventStream::new();
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let consumer = async {
            let text = |text: &str| {
                Ok(format!(
                    r#"{{"candidates":[{{"content":{{"parts":[{{"text":"{text}"}}]}}}}]}}"#
                ))
            };
            sender.send(text("before"))?;
            let started = events.next().await.ok_or("start")?;
            assert!(matches!(started, AssistantMessageEvent::TextStart { .. }));
            let delta = events.next().await.ok_or("delta")?;
            let retained = Arc::clone(partial(&delta).ok_or("partial")?);
            drop((started, delta));
            assert_eq!(first_text(&retained)?, "before");
            sender.send(text("after"))?;
            drop(events.next().await.ok_or("second delta")?);
            assert_eq!(first_text(&retained)?, "beforeafter");
            drop(sender);
            let ended = events.next().await.ok_or("end")?;
            assert!(
                matches!(&ended, AssistantMessageEvent::TextEnd { content, .. } if content == "beforeafter")
            );
            drop(ended);
            TestResult::Ok(retained)
        };
        let (result, retained) = futures_util::future::join(
            process_google_stream(
                Gate(receiver),
                &output,
                &events,
                &priced(None)?,
                &AtomicU64::new(0),
            ),
            consumer,
        )
        .await;
        result.map_err(|error| error.message)?;
        let retained = retained?;
        events.end(None);
        assert!(events.next().await.is_none());
        assert!(weak.upgrade().is_some());
        drop(output);
        assert!(weak.upgrade().is_some(), "the partial handle keeps it");
        drop(retained);
        drop(events);
        assert!(weak.upgrade().is_none());
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// A source that records how often it is polled.
struct Counted {
    /// Items still to yield.
    items: std::vec::IntoIter<Result<String, DiagnosticErrorInfo>>,
    /// Polls so far.
    polls: Arc<AtomicUsize>,
}

#[cfg(not(target_arch = "wasm32"))]
impl futures_core::Stream for Counted {
    type Item = Result<String, DiagnosticErrorInfo>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Poll::Ready(self.items.next())
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn source_diagnostics_return_unchanged_and_stop_polling() -> TestResult {
    let diagnostic = DiagnosticErrorInfo {
        name: Some("TransportError".to_owned()),
        message: "connection reset".to_owned(),
        stack: Some("at read".to_owned()),
        code: Some(DiagnosticCode::Text("ECONNRESET".to_owned())),
    };
    let polls = Arc::new(AtomicUsize::new(0));
    let source = Counted {
        items: vec![
            Ok(r#"{"candidates":[{"content":{"parts":[{"text":"partial"}]}}]}"#.to_owned()),
            Err(diagnostic.clone()),
            Ok("{}".to_owned()),
        ]
        .into_iter(),
        polls: Arc::clone(&polls),
    };
    let output = initial_message()?;
    let events = AssistantMessageEventStream::new();
    let result = crate::runtime::run::<crate::TestResult<_>>(async {
        Ok(
            process_google_stream(source, &output, &events, &priced(None)?, &AtomicU64::new(0))
                .await,
        )
    })?;
    assert_eq!(result, Err(diagnostic));
    assert_eq!(polls.load(Ordering::SeqCst), 2);
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn reducer_leaves_lifecycle_updates_to_its_caller() -> TestResult {
    crate::runtime::run(async {
        let output = initial_message()?;
        let events = AssistantMessageEventStream::new();
        let text = r#"{"candidates":[{"content":{"parts":[{"text":"hi"}]}}]}"#.to_owned();
        process_google_stream(
            futures_util::stream::iter(vec![Ok(text)]),
            &output,
            &events,
            &priced(None)?,
            &AtomicU64::new(0),
        )
        .await
        .map_err(|error| error.message)?;
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(events.next().await.ok_or("content event")?);
        }
        let types: Vec<_> = seen
            .iter()
            .map(|event| event_record(event).map(|record| record["type"].clone()))
            .collect::<TestResult<_>>()?;
        assert_eq!(types, ["text_start", "text_delta", "text_end"]);
        let mut next = Box::pin(events.next());
        let waiting = Pin::new(&mut next).poll(&mut Context::from_waker(Waker::noop()));
        assert!(waiting.is_pending(), "the reducer did not end the stream");
        events.push(AssistantMessageEvent::Done {
            reason: DoneReason::Stop,
            message: Arc::clone(&output),
        });
        let lifecycle = pin!(next).await.ok_or("lifecycle event")?;
        assert!(matches!(lifecycle, AssistantMessageEvent::Done { .. }));
        assert!(drain(&events).await?.is_empty());
        Ok(())
    })
}

#[cfg(not(target_arch = "wasm32"))]
/// Replay retained response fixtures with a fresh module-owned counter per invocation.
pub(crate) fn cloud_entry_rows() -> TestResult {
    let mut selected = Vec::new();
    for name in [
        "first_candidate_wins_without_reading_later_content",
        "thinking_switches_close_the_previous_block_in_order",
        "tool_calls_keep_server_ids_and_distinct_arguments",
        "pricing_uses_each_model_rate_with_source_expression_order",
    ] {
        selected.extend(rows(name)?);
    }
    let case = maestro_test_process::selection();
    if case.is_child() {
        return crate::runtime::run(async {
            let index = (0..selected.len())
                .find(|index| case.runs_case(&index.to_string()))
                .ok_or("unknown child case")?;
            cloud_entry_row(&selected[index]).await?;
            if index == 0 {
                crate::content_generation_invocation::native::live_alias("google-vertex").await?;
            }
            Ok(())
        });
    }
    for index in 0..selected.len() {
        {
            let output = maestro_test_process::TestProcess::current(
                "cloud_raw_entry_uses_shared_response_reduction",
            )?
            .nocapture()
            .env_clear()
            .case(index.to_string())
            .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
/// Observe one recorded response through framing, reduction and final settlement.
async fn cloud_entry_row(row: &Row) -> TestResult {
    use crate::content_generation_invocation::native;
    let mut model = priced(row.input.cost.as_ref())?;
    model.api = "google-vertex".into();
    model.provider = "google-vertex".into();
    let context = crate::bundled::conversation()?;
    let body = row
        .input
        .chunks
        .iter()
        .fold(String::new(), |mut body, chunk| {
            body.push_str("data: ");
            body.push_str(chunk);
            body.push_str("\n\n");
            body
        });
    let stream = maestro_models::stream_google_vertex(
        model,
        context,
        Some(maestro_models::GoogleVertexOptions {
            common: maestro_models::StreamOptions {
                api_key: Some("fixture-key".into()),
                fetch: Some(native::body_fetch(body.into_bytes())),
                ..Default::default()
            },
            ..Default::default()
        }),
    );
    let (mut events, message) = crate::response_output::collect(&stream).await?;
    assert_eq!(events.remove(0)["type"], "start");
    assert_eq!(events.pop().ok_or("terminal event")?["type"], "done");
    let mut selected = json!({"content":message["content"],"stopReason":message["stopReason"],"usage":message["usage"]});
    if let Some(id) = message.get("responseId") {
        selected["responseId"] = id.clone();
    }
    assert_row(
        row,
        Observed {
            message: selected,
            events,
            error: None,
        },
    )?;
    Ok(())
}
