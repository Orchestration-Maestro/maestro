//! Conversion of conversation history and tool declarations into response wire items.

use super::super::messages::{convert_responses_messages, convert_responses_tools};
use super::super::{ConvertResponsesMessagesOptions, ConvertResponsesToolsOptions};
use super::{TestResult, block_on, responses};
use crate as maestro_models;
use maestro_models::Tool;
use responses::{Fields, Script};
use serde_json::{Value, json};

/// Rows of conversion cases and the expectations they carry.
const REQUESTS: &str = include_str!("fixtures/requests.json");
/// Rows of reduction cases, checked here only for unique questions.
const EVENTS: &str = include_str!("fixtures/events.json");

/// Convert every message row a test owns.
fn message_rows(test: &str, count: usize) -> TestResult {
    responses::rows(REQUESTS, test, count)?
        .into_iter()
        .try_for_each(responses::check_messages)
}

#[test]
fn maestro_responses_messages_select_system_role() -> TestResult {
    message_rows("maestro_responses_messages_select_system_role", 12)
}

#[test]
fn maestro_responses_messages_preserve_user_content() -> TestResult {
    message_rows("maestro_responses_messages_preserve_user_content", 1)
}

#[test]
fn maestro_responses_messages_downgrade_unsupported_images() -> TestResult {
    message_rows("maestro_responses_messages_downgrade_unsupported_images", 1)
}

#[test]
fn maestro_responses_messages_keep_transformed_indices() -> TestResult {
    message_rows("maestro_responses_messages_keep_transformed_indices", 1)
}

#[test]
fn maestro_responses_messages_read_text_signatures() -> TestResult {
    message_rows("maestro_responses_messages_read_text_signatures", 15)
}

#[test]
fn maestro_responses_messages_bound_text_identity() -> TestResult {
    message_rows("maestro_responses_messages_bound_text_identity", 4)
}

#[test]
fn maestro_responses_messages_replay_signed_reasoning() -> TestResult {
    message_rows("maestro_responses_messages_replay_signed_reasoning", 6)
}

#[test]
fn maestro_responses_messages_omit_failed_history() -> TestResult {
    message_rows("maestro_responses_messages_omit_failed_history", 3)
}

#[test]
fn maestro_responses_messages_project_foreign_reasoning() -> TestResult {
    message_rows("maestro_responses_messages_project_foreign_reasoning", 10)
}

#[test]
fn maestro_responses_messages_normalize_tool_identity() -> TestResult {
    message_rows("maestro_responses_messages_normalize_tool_identity", 34)
}

#[test]
fn maestro_responses_messages_normalize_unlisted_provider() -> TestResult {
    message_rows("maestro_responses_messages_normalize_unlisted_provider", 1)
}

#[test]
fn maestro_responses_messages_keep_tool_association() -> TestResult {
    message_rows("maestro_responses_messages_keep_tool_association", 1)
}

#[test]
fn maestro_responses_messages_write_argument_json() -> TestResult {
    message_rows("maestro_responses_messages_write_argument_json", 1)?;
    let arguments: Value =
        serde_json::from_str(r#"{"10":10,"2":2,"z":9007199254740993,"zero":-0,"tiny":1e-7}"#)?;
    let history = responses::context(json!({"messages":[
        {"role":"assistant","content":[{"type":"toolCall","id":"c|fc_i","name":"lookup","arguments":arguments}]},
        {"role":"toolResult","toolCallId":"c|fc_i","content":[{"type":"text","text":"ok"}]}
    ]}))?;
    let items = convert_responses_messages(
        &responses::model(None)?,
        &history,
        &responses::allowed_providers(),
        None,
    )?;
    assert_eq!(
        items[0]["arguments"],
        r#"{"2":2,"10":10,"z":9007199254740992,"zero":0,"tiny":1e-7}"#
    );
    Ok(())
}

#[test]
fn maestro_responses_messages_keep_tool_result_images() -> TestResult {
    message_rows("maestro_responses_messages_keep_tool_result_images", 10)
}

#[test]
fn maestro_responses_tools_preserve_strict_states() -> TestResult {
    responses::rows(
        REQUESTS,
        "maestro_responses_tools_preserve_strict_states",
        8,
    )?
    .into_iter()
    .try_for_each(responses::check_tools)
}

#[test]
fn maestro_responses_options_default_to_prompt_and_strict_false() -> TestResult {
    let history = responses::context(json!({
        "systemPrompt": "rules",
        "messages": [{"role": "user", "content": "hello"}]
    }))?;
    let model = responses::model(None)?;
    let providers = responses::allowed_providers();
    let by_default = convert_responses_messages(
        &model,
        &history,
        &providers,
        Some(&ConvertResponsesMessagesOptions::default()),
    )?;
    let unspecified = convert_responses_messages(&model, &history, &providers, None)?;
    assert_eq!(by_default, unspecified);
    assert_eq!(
        by_default[0],
        json!({"role": "developer", "content": "rules"})
    );

    let tools: Vec<Tool> = serde_json::from_value(json!([
        {"name": "lookup", "description": "find", "parameters": {"type": "object"}}
    ]))?;
    let declared = convert_responses_tools(&tools, Some(&ConvertResponsesToolsOptions::default()));
    assert_eq!(declared[0]["strict"], json!(false));
    Ok(())
}

/// `containers` arrays nested around `0`.
fn nested(containers: usize) -> String {
    format!("{}0{}", "[".repeat(containers), "]".repeat(containers))
}

#[test]
fn maestro_responses_messages_bound_full_signature_conversion() -> TestResult {
    let replay = |depth: usize| {
        let signature = nested(depth);
        let history = responses::context(json!({"messages": [{
            "role": "assistant",
            "content": [{"type": "thinking", "thinking": "", "thinkingSignature": signature}]
        }]}))?;
        let items = convert_responses_messages(
            &responses::model(None)?,
            &history,
            &responses::allowed_providers(),
            None,
        );
        Ok::<_, Box<dyn std::error::Error>>((signature, items))
    };
    let (signature, accepted) = replay(127)?;
    assert_eq!(serde_json::to_string(&accepted?)?, format!("[{signature}]"));
    let (_, rejected) = replay(128)?;
    let failure = rejected.err().ok_or("128 containers are rejected")?;
    assert_eq!(failure.message, "recursion limit exceeded");

    let reduce_item = |depth: usize| {
        let item = format!(
            r#"{{"type":"reasoning","id":"rs_1","summary":[{{"text":"final"}}],"future":{}}}"#,
            nested(depth - 1)
        );
        let added = r#"{"type":"response.output_item.added","item":{"type":"reasoning","id":"rs_1","summary":[]}}"#;
        let done = format!(r#"{{"type":"response.output_item.done","item":{item}}}"#);
        let completed = r#"{"type":"response.completed","response":{"status":"completed"}}"#;
        let script = Script::of(vec![
            added.to_owned(),
            r#"{"type":"response.reasoning_text.delta","delta":"draft"}"#.to_owned(),
            done,
            completed.to_owned(),
        ])?;
        Ok::<_, Box<dyn std::error::Error>>((item, block_on(false, responses::reduce(script))?))
    };
    let (item, accepted) = reduce_item(127)?;
    accepted.outcome?;
    let kept = accepted.message.content.first().ok_or("one block")?;
    assert_eq!(serde_json::to_value(kept)?["thinkingSignature"], item);
    let (_, rejected) = reduce_item(128)?;
    let failure = rejected
        .outcome
        .err()
        .ok_or("128 containers are rejected")?;
    assert_eq!(failure.message, "recursion limit exceeded");
    assert_eq!(
        serde_json::to_value(&rejected.message.content)?,
        json!([{"type": "thinking", "thinking": "final"}]),
        "final content survives signature failure unsigned"
    );
    assert_eq!(
        rejected.events,
        [
            json!({"type": "thinking_start", "contentIndex": 0}),
            json!({"type":"thinking_delta","contentIndex":0,"delta":"draft"})
        ]
    );
    Ok(())
}

#[test]
fn maestro_responses_fixtures_hold_unique_queries_and_reject_unread_members() -> TestResult {
    responses::assert_unique_queries(REQUESTS)?;
    responses::assert_unique_queries(EVENTS)?;

    let rows: Vec<Value> = serde_json::from_str(EVENTS)?;
    let sample = rows.first().cloned().ok_or("the fixture has rows")?;
    let mut again = sample.clone();
    again["id"] = json!("again");
    let repeated = Value::Array(rows.into_iter().chain([again]).collect());
    assert!(responses::assert_unique_queries(&repeated.to_string()).is_err());
    let mut raw_copy = sample.clone();
    raw_copy["id"] = json!("raw-copy");
    for event in raw_copy["events"]
        .as_array_mut()
        .ok_or("events are an array")?
    {
        *event = Value::String(event.to_string());
    }
    assert!(responses::assert_unique_queries(&json!([sample, raw_copy]).to_string()).is_err());

    let consume = |row: Value| {
        let mut fields = Fields::new("sample", row)?;
        fields.take("test");
        fields.take("id");
        block_on(false, responses::check_events(fields))
    };
    consume(sample.clone())?;
    let mut nested = sample.clone();
    nested["events"][0]["response"]["unused"] = json!(1);
    let failure = consume(nested)
        .err()
        .ok_or("the unread nested member is rejected")?;
    assert!(failure.to_string().contains("unread members"), "{failure}");
    let mut raw_nested = sample.clone();
    raw_nested["events"][0]["response"]["unused"] = json!(1);
    raw_nested["events"][0] = Value::String(raw_nested["events"][0].to_string());
    let failure = consume(raw_nested)
        .err()
        .ok_or("raw nested member is unread")?;
    assert!(failure.to_string().contains("unread members"), "{failure}");
    let call = json!({"type":"toolCall","id":"c|i","name":"lookup","arguments":{}});
    let mut call_row = json!({"events":[
        {"type":"response.output_item.added","item":{"type":"function_call","id":"i","call_id":"c","name":"lookup","arguments":"{}"}},
        {"type":"response.output_item.done","item":{"type":"function_call"}},
        {"type":"response.completed"}
    ],"expected":{"result":{"content":[call],"stopReason":"toolUse"},"events":[
        {"type":"toolcall_start","contentIndex":0},
        {"type":"toolcall_end","contentIndex":0,"toolCall":call}
    ]}});
    consume(call_row.clone())?;
    call_row["events"][1]["item"]["arguments"] = json!("{}");
    let failure = consume(call_row)
        .err()
        .ok_or("nonempty scratch leaves final arguments unread")?;
    assert!(failure.to_string().contains("unread members"), "{failure}");
    let mut extended = sample;
    extended["unused"] = json!(1);
    let failure = consume(extended)
        .err()
        .ok_or("the unread member is rejected")?;
    assert!(failure.to_string().contains("unread members"), "{failure}");
    Ok(())
}
