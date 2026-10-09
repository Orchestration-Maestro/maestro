//! Strict fixture records with generated repeated text.

use serde::Deserialize;
use serde_json::Value;

/// One repeated run inside a generated string.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepeatedText {
    /// Literal prefix.
    prefix: String,
    /// Repeated unit.
    repeat: String,
    /// Number of repetitions.
    count: usize,
    /// Literal suffix.
    suffix: String,
}

/// Expand generated inputs before executing the actual operation.
pub(super) fn expand(value: &mut Value) {
    match value {
        Value::Object(members) if members.contains_key("generated_text") => {
            assert_eq!(members.len(), 1);
            let text: RepeatedText =
                serde_json::from_value(members.remove("generated_text").unwrap()).unwrap();
            *value = Value::String(format!(
                "{}{}{}",
                text.prefix,
                text.repeat.repeat(text.count),
                text.suffix
            ));
        }
        Value::Object(members) => members.values_mut().for_each(expand),
        Value::Array(values) => values.iter_mut().for_each(expand),
        _ => {}
    }
}

/// Reject unread expectation fields instead of silently accepting recording metadata.
pub(super) fn expectation(operation: &str, expected: &Value) {
    let fields: &[&str] = match operation {
        "payload" | "simple_payload" => &["payload", "wire", "wire_error"],
        "negative_zero" => &["temperatureNegative", "tokensNegative", "wire"],
        "request_options" => &["headers", "headerKeys"],
        "endpoint" => &["ok", "url", "message", "native_error"],
        "media_type" => &["sse", "json"],
        "send" => &["requests", "hook_calls", "response_calls", "outcome"],
        "format_error" | "derive_id" | "normalize_ids" => return,
        other => panic!("unknown operation {other}"),
    };
    keys(expected, fields);
    if operation == "send" {
        keys(
            &expected["outcome"],
            &[
                "admitted",
                "error",
                "error_prefix",
                "native_error",
                "native_cause",
            ],
        );
        for request in expected["requests"].as_array().unwrap() {
            keys(request, &["url", "method", "headers", "body", "aborted"]);
        }
    }
}

/// Every accepted key belongs to the consumed operation record.
fn keys(value: &Value, fields: &[&str]) {
    for key in value.as_object().unwrap().keys() {
        assert!(
            fields.contains(&key.as_str()),
            "unread expectation field {key}"
        );
    }
}
