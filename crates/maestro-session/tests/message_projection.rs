//! Recorded message transformation cases.
#![cfg(test)]
use chrono::{TimeZone, Timelike};
use maestro_session::messages::*;
use serde::Deserialize;
use serde_json::Value;

/// One recorded function invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Failure label.
    id: String,
    /// Invoked operation.
    operation: String,
    /// Function arguments.
    args: Vec<Value>,
    /// Recorded result.
    expected: Value,
    /// Owning witness.
    test: String,
}

/// Evaluate shell cases in one group.
fn shell_group(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/message_projection.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        assert_eq!(case.operation, "bashExecutionToText");
        let message = serde_json::from_value(decode(case.args[0].clone())).unwrap();
        assert_eq!(
            bash_execution_to_text(&message),
            case.expected.as_str().unwrap(),
            "{}",
            case.id
        );
        count += 1;
    }
    assert!(count > 0);
}

#[test]
fn shell_transcript_keeps_authored_text() {
    shell_group("shell_transcript_keeps_authored_text");
}

#[test]
fn shell_status_uses_cancellation_before_exit() {
    shell_group("shell_status_uses_cancellation_before_exit");
}

/// Decode transport markers without changing ordinary values.
fn decode(value: Value) -> Value {
    match value {
        Value::Object(mut object) => {
            if object.get("$number").and_then(Value::as_str) == Some("-0") {
                return serde_json::json!(-0.0);
            }
            object.retain(|_, value| value.get("$undefined") != Some(&Value::Bool(true)));
            Value::Object(
                object
                    .into_iter()
                    .map(|(key, value)| (key, decode(value)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.into_iter().map(decode).collect()),
        Value::Number(number) => serde_json::json!(number.as_f64().unwrap()),
        value => value,
    }
}

#[test]
fn shell_truncation_requires_flag_and_path() {
    shell_group("shell_truncation_requires_flag_and_path");
}

/// Select recorded cases for a witness.
fn cases(name: &str) -> Vec<Case> {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/message_projection.json")).unwrap();
    let selected: Vec<_> = cases.into_iter().filter(|case| case.test == name).collect();
    assert!(!selected.is_empty());
    selected
}

/// Run date cases in a fresh process with the selected timezone.
#[cfg(not(target_arch = "wasm32"))]
fn in_timezone(name: &str, timezone: &str) -> bool {
    if maestro_test_process::selection().is_child()
        && maestro_test_process::selection().runs_case(name)
    {
        return true;
    }
    if !maestro_test_process::local_zone_follows_tz() {
        return false;
    }
    let output = maestro_test_process::TestProcess::current(name)
        .unwrap()
        .nocapture()
        .env("TZ", timezone)
        .case(name)
        .output()
        .unwrap();
    assert!(output.status.success());

    false
}

/// Compare fields and timestamp, including invalid time, then wire encoding.
fn assert_constructor(case: &Case, actual: Value, timestamp: f64) {
    let mut expected = decode(case.expected.clone());
    if expected["timestamp"]["$number"] == "NaN" {
        assert!(timestamp.is_nan(), "{}", case.id);
        expected["timestamp"] = Value::Null;
    } else {
        assert_eq!(
            timestamp.to_bits(),
            expected["timestamp"].as_f64().unwrap().to_bits(),
            "{}",
            case.id
        );
    }
    assert_eq!(decode(actual), expected, "{}", case.id);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn summary_constructors_preserve_fields_and_time() {
    let name = "summary_constructors_preserve_fields_and_time";
    if !in_timezone(name, "UTC") {
        return;
    }
    for case in cases(name) {
        let summary = case.args[0].as_str().unwrap().to_owned();
        let time = case.args[2].as_str().unwrap();
        match case.operation.as_str() {
            "createBranchSummaryMessage" => {
                let message = create_branch_summary_message(
                    summary,
                    case.args[1].as_str().unwrap().to_owned(),
                    time,
                );
                assert_constructor(
                    &case,
                    serde_json::to_value(&message).unwrap(),
                    message.timestamp,
                );
            }
            "createCompactionSummaryMessage" => {
                let message = create_compaction_summary_message(
                    summary,
                    case.args[1].as_f64().unwrap(),
                    time,
                );
                assert_constructor(
                    &case,
                    serde_json::to_value(&message).unwrap(),
                    message.timestamp,
                );
            }
            _ => panic!("unexpected operation"),
        }
    }
}

use maestro_request::presence::Presence;

/// Invoke the custom constructor for each selected recorded query.
fn custom_factory_group(name: &str) {
    for case in cases(name) {
        assert_eq!(case.operation, "createCustomMessage");
        let details = if case.args[3].get("$undefined") == Some(&Value::Bool(true)) {
            Presence::Missing
        } else {
            serde_json::from_value(case.args[3].clone()).unwrap()
        };
        let message = create_custom_message::<Value>(
            case.args[0].as_str().unwrap().to_owned(),
            serde_json::from_value(case.args[1].clone()).unwrap(),
            case.args[2].as_bool().unwrap(),
            details,
            case.args[4].as_str().unwrap(),
        );
        assert_constructor(
            &case,
            serde_json::to_value(&message).unwrap(),
            message.timestamp,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn constructors_use_local_datetimes_and_utc_dates() {
    let name = "constructors_use_local_datetimes_and_utc_dates";
    if !in_timezone(name, "America/New_York") {
        return;
    }
    for (input, expected) in [
        ("2025-03-04T05:06:07.008", 1_741_082_767_008.0_f64),
        ("2025-03-04", 1_741_046_400_000.0),
    ] {
        let branch = create_branch_summary_message("branch".into(), "from".into(), input);
        let compaction = create_compaction_summary_message("compaction".into(), 19.5, input);
        let custom = create_custom_message::<Value>(
            "note".into(),
            maestro_models::UserContent::Text("custom".into()),
            false,
            Presence::Missing,
            input,
        );
        assert_eq!(
            branch.timestamp.to_bits(),
            expected.to_bits(),
            "branch: {input}"
        );
        assert_eq!(
            compaction.timestamp.to_bits(),
            expected.to_bits(),
            "compaction: {input}"
        );
        assert_eq!(
            custom.timestamp.to_bits(),
            expected.to_bits(),
            "custom: {input}"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn custom_constructor_preserves_time() {
    let name = "custom_constructor_preserves_time";
    if in_timezone(name, "UTC") {
        custom_factory_group(name);
    }
}

#[test]
fn custom_constructor_keeps_content_visibility_and_details() {
    /// Payload without Clone, Default or serialization requirements.
    struct Payload(u8);
    custom_factory_group("custom_constructor_keeps_content_visibility_and_details");
    let message = create_custom_message(
        "typed".into(),
        maestro_models::UserContent::Text("payload".into()),
        false,
        Presence::Present(Payload(9)),
        "1970-01-01",
    );
    assert!(matches!(message.details, Presence::Present(Payload(9))));
}

use maestro_agent::{AgentMessage, CustomAgentMessages as _};
use maestro_models::Message;
use std::sync::{Arc, RwLock};

/// Unknown caller role.
struct Unknown;
impl maestro_agent::CustomAgentMessages for Unknown {
    fn role(&self) -> &'static str {
        "future.notice"
    }
}
/// Test conversation entry.
type Entry = AgentMessage<CustomAgentMessages<Value, Unknown>>;

/// Decode one fixture entry through its owning record.
fn entry(value: Value) -> Entry {
    let value = decode(value);
    match value["role"].as_str().unwrap() {
        "user" | "assistant" | "toolResult" => match serde_json::from_value(value).unwrap() {
            Message::User(message) => Entry::User(Arc::new(RwLock::new(message))),
            Message::Assistant(message) => Entry::Assistant(Arc::new(RwLock::new(message))),
            Message::ToolResult(message) => Entry::ToolResult(Arc::new(RwLock::new(message))),
        },
        role => {
            let message = match role {
                "custom" => CustomAgentMessages::Custom(serde_json::from_value(value).unwrap()),
                "bashExecution" => {
                    CustomAgentMessages::BashExecution(serde_json::from_value(value).unwrap())
                }
                "branchSummary" => {
                    CustomAgentMessages::BranchSummary(serde_json::from_value(value).unwrap())
                }
                "compactionSummary" => {
                    CustomAgentMessages::CompactionSummary(serde_json::from_value(value).unwrap())
                }
                _ => CustomAgentMessages::Other(Unknown),
            };
            Entry::Custom(Arc::new(RwLock::new(message)))
        }
    }
}

/// Compare the entire ordered model transcript for each recorded invocation.
fn conversion_group(name: &str) {
    for case in cases(name) {
        assert_eq!(case.operation, "convertToLlm");
        let mut entries: Vec<_> = case.args[0]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(entry)
            .collect();
        if case.id == "base-sequence" {
            entries[3] = entries[1].clone();
        }
        assert_eq!(
            decode(serde_json::to_value(convert_to_llm(&entries)).unwrap()),
            decode(case.expected),
            "{}",
            case.id
        );
    }
}

#[test]
fn custom_projection_keeps_block_order_and_ignores_visibility() {
    conversion_group("custom_projection_keeps_block_order_and_ignores_visibility");
}

#[test]
fn summary_projection_keeps_distinct_newline_wrappers() {
    conversion_group("summary_projection_keeps_distinct_newline_wrappers");
}

#[test]
fn conversion_keeps_base_values_order_and_duplicates() {
    conversion_group("conversion_keeps_base_values_order_and_duplicates");
}

#[test]
fn conversion_drops_only_suppressed_shell_and_unknown_roles() {
    conversion_group("conversion_drops_only_suppressed_shell_and_unknown_roles");
    for (value, role) in [
        (
            serde_json::json!({"role":"bashExecution","command":"","output":"","cancelled":false,"truncated":false,"timestamp":0}),
            "bashExecution",
        ),
        (
            serde_json::json!({"role":"custom","customType":"x","content":"","display":false,"timestamp":0}),
            "custom",
        ),
        (
            serde_json::json!({"role":"branchSummary","summary":"","fromId":"x","timestamp":0}),
            "branchSummary",
        ),
        (
            serde_json::json!({"role":"compactionSummary","summary":"","tokensBefore":0,"timestamp":0}),
            "compactionSummary",
        ),
        (serde_json::json!({"role":"future.notice"}), "future.notice"),
    ] {
        let Entry::Custom(message) = entry(value) else {
            panic!("expected custom entry")
        };
        assert_eq!(message.read().unwrap().role(), role);
    }
}

#[test]
fn conversion_observes_changes_through_retained_inputs() {
    let rows = cases("conversion_observes_changes_through_retained_inputs");
    assert_eq!(rows.len(), 2);
    let input = entry(rows[0].args[0][0].clone());
    for (index, case) in rows.into_iter().enumerate() {
        assert_eq!(case.operation, "convertToLlm");
        if index == 1 {
            let Entry::User(message) = &input else {
                panic!("expected user")
            };
            message.write().unwrap().content =
                serde_json::from_value(case.args[0][0]["content"].clone()).unwrap();
        }
        assert_eq!(
            decode(
                serde_json::to_value(maestro_session::convert_to_llm(std::slice::from_ref(
                    &input
                )))
                .unwrap()
            ),
            decode(case.expected),
            "{}",
            case.id
        );
    }
}

/// Recorded calendar parsing across isolated local time zones.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn parse_timestamp_millis_preserves_native_date_forms() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ParserCase {
        test: String,
        input: ParserInput,
        expected: ParserExpected,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ParserInput {
        tz: String,
        text: String,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ParserExpected {
        millis: Option<i64>,
    }
    if !maestro_test_process::selection().is_child() {
        if !maestro_test_process::local_zone_follows_tz() {
            return;
        }
        for tz in ["UTC", "America/New_York", "Asia/Kathmandu"] {
            crate::subprocess::run_child(
                maestro_test_process::TestProcess::current(
                    "parse_timestamp_millis_preserves_native_date_forms",
                )
                .unwrap()
                .case("*"),
                &[("TZ", Some(tz))],
            );
        }
        return;
    }
    let tz = std::env::var("TZ").unwrap();
    let cases: Vec<ParserCase> =
        serde_json::from_str(include_str!("fixtures/timestamp-parser.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|c| c.input.tz == tz) {
        assert_eq!(
            case.test,
            "parse_timestamp_millis_preserves_native_date_forms"
        );
        assert_eq!(
            parse_timestamp_millis(&case.input.text),
            case.expected.millis
        );
        count += 1;
    }
    assert!(count > 0);
}

/// One recorded local-time reading and its resolved instant.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalCase {
    /// Zone the case runs in.
    zone: String,
    /// Local wall time without an offset.
    wall: String,
    /// Resolved epoch milliseconds.
    millis: i64,
    /// Local date the resolved instant falls on.
    local_date: String,
    /// Local hour the resolved instant falls on.
    local_hour: u32,
}

#[test]
fn local_timestamps_keep_gap_overlap_and_calendar_boundaries() {
    if !maestro_test_process::selection().is_child() {
        if !maestro_test_process::local_zone_follows_tz() {
            return;
        }
        for tz in [
            "America/New_York",
            "America/Santiago",
            "America/Havana",
            "Pacific/Apia",
            "America/Montreal",
            "Europe/Berlin",
        ] {
            crate::subprocess::run_child(
                maestro_test_process::TestProcess::current(
                    "local_timestamps_keep_gap_overlap_and_calendar_boundaries",
                )
                .unwrap()
                .case("*"),
                &[("TZ", Some(tz))],
            );
        }
        return;
    }
    let tz = std::env::var("TZ").unwrap();
    let cases: Vec<LocalCase> =
        serde_json::from_str(include_str!("fixtures/local-timestamps.json")).unwrap();
    let mut checked = 0;
    for case in cases.iter().filter(|case| case.zone == tz) {
        let wall = chrono::NaiveDateTime::parse_from_str(&case.wall, "%Y-%m-%dT%H:%M:%S")
            .expect("the recorded wall time parses");
        let millis = local_timestamp_millis(wall).expect("the local reading resolves");
        assert_eq!(millis, case.millis, "{}", case.wall);
        assert_eq!(
            parse_timestamp_millis(&case.wall),
            Some(case.millis),
            "the timestamp parser agrees on {}",
            case.wall
        );
        let local = chrono::Local
            .timestamp_millis_opt(millis)
            .single()
            .expect("the resolved instant has one local reading");
        assert_eq!(local.format("%Y-%m-%d").to_string(), case.local_date);
        assert_eq!(local.hour(), case.local_hour);
        checked += 1;
    }
    assert!(checked > 0, "the zone has recorded cases");
}
