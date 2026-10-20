//! Cloud endpoint stream lifecycle through controlled public invocations.
use crate::azure_endpoint as endpoint;
use crate::chat;
use chat::TestResult;
use serde_json::{Value, json};

#[test]
fn azure_sequence_hooks_and_preserve_failures() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_sequence_hooks_and_preserve_failures",
    ))
}

#[test]
fn azure_keep_failure_precedence_before_transport() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_keep_failure_precedence_before_transport",
    ))
}

#[test]
fn azure_finish_terminal_outcomes_with_partial_identity() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_finish_terminal_outcomes_with_partial_identity",
    ))
}

#[test]
fn azure_retain_model_identity_usage_and_unscaled_cost() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_retain_model_identity_usage_and_unscaled_cost",
    ))
}

#[test]
fn azure_preserve_abort_stage_and_competing_error() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_preserve_abort_stage_and_competing_error",
    ))
}

#[test]
fn azure_reuse_response_framing_and_error_selection() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_reuse_response_framing_and_error_selection",
    ))
}

#[test]
fn azure_return_http_errors_and_detect_overflow() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_return_http_errors_and_detect_overflow",
    ))
}

use crate::azure_cases;

use crate::response_runtime as runtime;

#[test]
fn azure_gate_hooks_and_cancel_partial_output() -> chat::TestResult {
    crate::runtime::run(runtime::gated(runtime::Endpoint::Azure))
}

#[test]
fn azure_use_default_transport_for_native_results() -> chat::TestResult {
    crate::runtime::run(runtime::native(runtime::Endpoint::Azure))
}

#[test]
fn azure_forward_retry_timeout_and_signal_options() -> chat::TestResult {
    crate::runtime::run_paused(runtime::transport_options(runtime::Endpoint::Azure))
}

#[test]
fn azure_finish_without_a_native_executor() -> chat::TestResult {
    runtime::without_runtime(runtime::Endpoint::Azure)
}

#[test]
fn azure_fixtures_have_unique_complete_consumers() -> chat::TestResult {
    runtime::fixture_consumers(
        endpoint::FIXTURE,
        azure_cases::rows,
        concat!(
            include_str!("azure_endpoint_requests.rs"),
            include_str!("azure_endpoint_streams.rs")
        ),
        (28, 327),
        normalized_query,
    )
}

/// Canonical defaults make omitted and explicitly defaulted inputs the same query.
fn normalized_query(case: &Value) -> TestResult<String> {
    let mut query = case.clone();
    let object = query.as_object_mut().ok_or("case object")?;
    object
        .shift_remove("entry")
        .filter(|v| v != "raw")
        .map(|v| object.insert("entry".into(), v));
    for field in ["model", "options", "env", "context"] {
        if query[field].is_null() {
            query[field] = json!({});
        }
    }
    let defaults = [
        ("model", json!({"reasoning":true,"thinkingLevelMap":{}})),
        ("options", json!({"apiKey":"test-key","maxRetries":0})),
    ];
    for (field, defaults) in defaults {
        let target = query[field].as_object_mut().ok_or("query object")?;
        for (key, value) in defaults.as_object().ok_or("defaults")? {
            if target.get(key).is_some_and(|v| v == value) {
                target.shift_remove(key);
            }
        }
    }
    Ok(serde_json::to_string(&sort_query(query))?)
}
/// Stable member ordering for query identity, never for observed request ordering.
fn sort_query(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .map(|(k, v)| (k, sort_query(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(sort_query).collect()),
        other => other,
    }
}
