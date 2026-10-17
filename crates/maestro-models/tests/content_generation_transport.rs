//! Pure content-generation transport preparation against recorded request observations.
#![allow(
    dead_code,
    reason = "shared chat support serves multiple test binaries"
)]

use crate::chat;

use std::collections::HashSet;

use chat::TestResult;
use indexmap::IndexMap;
use maestro_models::providers::content::google::create_client;
use maestro_models::providers::content::google_shared::{
    ApiClient, PreparedContentRequest, project_content_request,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Behavior owners of every committed row.
const TESTS: &[&str] = &[
    "default_endpoint_uses_the_public_api_version",
    "model_endpoints_append_no_implicit_version",
    "request_endpoint_and_version_override_independently",
    "resource_text_is_encoded_only_by_url_construction",
    "url_components_keep_constructor_and_stream_query_rules",
    "invalid_request_endpoints_fail_before_headers_or_auth",
    "projection_errors_precede_transport_option_errors",
    "same_spelling_headers_replace_across_all_layers",
    "differently_spelled_headers_append_after_layering",
    "caller_metadata_and_empty_headers_survive",
    "header_normalization_keeps_byte_values_without_native_preflight",
    "invalid_headers_fail_before_key_validation",
    "replaced_invalid_header_values_are_not_read",
    "case_variants_do_not_hide_invalid_earlier_values",
    "api_key_insertion_keeps_header_and_token_rules",
    "authored_api_keys_override_insertion_but_not_token_rejection",
    "timeouts_retain_milliseconds_and_ceil_the_server_header",
    "server_timeout_headers_append_after_validation",
    "absent_options_and_unused_controls_do_not_change_the_request",
    "extra_body_merges_objects_and_replaces_nonobjects",
    "extra_body_retains_open_values_and_type_changes",
    "extra_body_bypasses_projection_without_redirecting_the_resource",
    "extra_body_cannot_repair_an_earlier_projection_failure",
    "merged_body_text_preserves_key_positions_and_binary64_numbers",
    "client_snapshots_inputs_and_preparations_do_not_mutate_controls",
];

/// One recorded input and its complete observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Owning behavior test.
    test: String,
    /// Exact input JSON.
    input: String,
    /// Expected observation.
    expected: Expected,
}

/// Successful request or failure category.
#[derive(Deserialize)]
#[serde(untagged)]
enum Expected {
    /// Complete prepared request.
    Request(Observation),
    /// First failing phase.
    Failure(Failure),
}

/// Recorded failure phase.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Failure {
    /// Projection, URL, header or authentication.
    error_kind: String,
}

/// Complete transport observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    /// Final URL.
    url: String,
    /// Request method.
    method: String,
    /// Ordered normalized entries.
    headers: Vec<(String, String)>,
    /// Exact body text.
    body: String,
    /// Positive milliseconds.
    timeout: Option<f64>,
    /// Second preparation on the retained client.
    followup: Option<Box<Observation>>,
}

/// Supplied settings and selected payload.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Constructor endpoint.
    base_url: String,
    /// Supplied key.
    api_key: String,
    /// Selected payload.
    params: Value,
    /// Model headers.
    model_headers: Option<IndexMap<String, String>>,
    /// Option headers.
    option_headers: Option<IndexMap<String, String>>,
    /// Whether to change constructor inputs and prepare again.
    #[serde(default)]
    snapshot: bool,
}

/// Move client-only controls before consuming the selected payload.
fn prepare(
    client: &ApiClient,
    mut params: Value,
) -> Result<PreparedContentRequest, maestro_models::DiagnosticErrorInfo> {
    let controls = params
        .get_mut("config")
        .and_then(Value::as_object_mut)
        .and_then(|config| config.shift_remove("httpOptions"));
    let projected = project_content_request(params)?;
    chat::block_on(false, async {
        Ok(client.prepare_request(projected, controls.as_ref()).await)
    })
    .map_err(|error| maestro_models::DiagnosticErrorInfo::error(error.to_string()))?
}

/// Compare every returned transport field without printing header values.
fn check_request(actual: PreparedContentRequest, expected: &Observation) {
    assert_eq!(actual.request.url, expected.url);
    assert_eq!(actual.request.method, expected.method);
    let headers: Vec<_> = actual.request.headers.into_iter().collect();
    assert!(headers == expected.headers, "ordered headers differ");
    assert_eq!(actual.request.body, expected.body.as_bytes());
    assert_eq!(actual.timeout, expected.timeout);
    assert!(actual.request.signal.is_none());
}

/// Consume all assigned rows, rejecting duplicate inputs and unknown fixture members.
fn assert_rows(test: &str) -> TestResult {
    let fixture = include_str!("fixtures/content_transport.json");
    let all: Vec<Row> = serde_json::from_str(fixture)?;
    let mut inputs = HashSet::new();
    for row in &all {
        assert!(
            TESTS.contains(&row.test.as_str()),
            "unassigned transport row"
        );
        assert!(inputs.insert(&row.input), "duplicate transport input");
    }
    assert!(
        TESTS
            .iter()
            .all(|name| all.iter().any(|row| row.test == *name))
    );
    let selected = chat::rows(fixture, test)?;
    assert!(!selected.is_empty());
    for row in selected {
        check_row(serde_json::from_value(row)?)?;
    }
    Ok(())
}

/// Observe the selected request and the retained-client followup when present.
fn check_row(row: Row) -> TestResult {
    let mut input: Input = serde_json::from_str(&row.input)?;
    let mut model =
        chat::model(&json!({"baseUrl": input.base_url, "headers": input.model_headers}))?;
    let client = create_client(&model, &input.api_key, input.option_headers.as_ref(), None);
    if input.snapshot {
        "https://decoy.invalid".clone_into(&mut model.base_url);
        model.headers = Some(IndexMap::from([("X".to_owned(), "changed".to_owned())]));
        if let Some(headers) = &mut input.option_headers {
            headers.insert("Y".to_owned(), "changed".to_owned());
        }
        "changed-key".clone_into(&mut input.api_key);
    }
    let actual = prepare(&client, input.params);
    match row.expected {
        Expected::Request(expected) => {
            check_request(actual?, &expected);
            assert_eq!(input.snapshot, expected.followup.is_some());
            if let Some(followup) = expected.followup {
                check_request(
                    prepare(
                        &client,
                        json!({"model": "gemini-target", "contents": "next prompt"}),
                    )?,
                    &followup,
                );
                assert!(followup.followup.is_none());
            }
        }
        Expected::Failure(expected) => {
            let error = actual.err().ok_or("expected preparation failure")?;
            let phase = if error.message.starts_with("URL:") {
                "url"
            } else if error.message.starts_with("Header:") {
                "header"
            } else if error.message.starts_with("Authentication:") {
                "auth"
            } else {
                "projection"
            };
            assert_eq!(phase, expected.error_kind);
            assert!(!input.snapshot);
        }
    }
    Ok(())
}

#[test]
fn default_endpoint_uses_the_public_api_version() -> TestResult {
    assert_rows("default_endpoint_uses_the_public_api_version")
}

#[test]
fn model_endpoints_append_no_implicit_version() -> TestResult {
    assert_rows("model_endpoints_append_no_implicit_version")
}

#[test]
fn request_endpoint_and_version_override_independently() -> TestResult {
    assert_rows("request_endpoint_and_version_override_independently")
}

#[test]
fn resource_text_is_encoded_only_by_url_construction() -> TestResult {
    assert_rows("resource_text_is_encoded_only_by_url_construction")
}

#[test]
fn url_components_keep_constructor_and_stream_query_rules() -> TestResult {
    assert_rows("url_components_keep_constructor_and_stream_query_rules")
}

#[test]
fn invalid_request_endpoints_fail_before_headers_or_auth() -> TestResult {
    assert_rows("invalid_request_endpoints_fail_before_headers_or_auth")
}

#[test]
fn projection_errors_precede_transport_option_errors() -> TestResult {
    assert_rows("projection_errors_precede_transport_option_errors")
}

#[test]
fn same_spelling_headers_replace_across_all_layers() -> TestResult {
    assert_rows("same_spelling_headers_replace_across_all_layers")
}

#[test]
fn differently_spelled_headers_append_after_layering() -> TestResult {
    assert_rows("differently_spelled_headers_append_after_layering")
}

#[test]
fn caller_metadata_and_empty_headers_survive() -> TestResult {
    assert_rows("caller_metadata_and_empty_headers_survive")
}

#[test]
fn header_normalization_keeps_byte_values_without_native_preflight() -> TestResult {
    assert_rows("header_normalization_keeps_byte_values_without_native_preflight")
}

#[test]
fn invalid_headers_fail_before_key_validation() -> TestResult {
    assert_rows("invalid_headers_fail_before_key_validation")
}

#[test]
fn replaced_invalid_header_values_are_not_read() -> TestResult {
    assert_rows("replaced_invalid_header_values_are_not_read")
}

#[test]
fn case_variants_do_not_hide_invalid_earlier_values() -> TestResult {
    assert_rows("case_variants_do_not_hide_invalid_earlier_values")
}

#[test]
fn api_key_insertion_keeps_header_and_token_rules() -> TestResult {
    assert_rows("api_key_insertion_keeps_header_and_token_rules")
}

#[test]
fn authored_api_keys_override_insertion_but_not_token_rejection() -> TestResult {
    assert_rows("authored_api_keys_override_insertion_but_not_token_rejection")
}

#[test]
fn timeouts_retain_milliseconds_and_ceil_the_server_header() -> TestResult {
    assert_rows("timeouts_retain_milliseconds_and_ceil_the_server_header")
}

#[test]
fn server_timeout_headers_append_after_validation() -> TestResult {
    assert_rows("server_timeout_headers_append_after_validation")
}

#[test]
fn absent_options_and_unused_controls_do_not_change_the_request() -> TestResult {
    assert_rows("absent_options_and_unused_controls_do_not_change_the_request")
}

#[test]
fn extra_body_merges_objects_and_replaces_nonobjects() -> TestResult {
    assert_rows("extra_body_merges_objects_and_replaces_nonobjects")
}

#[test]
fn extra_body_retains_open_values_and_type_changes() -> TestResult {
    assert_rows("extra_body_retains_open_values_and_type_changes")
}

#[test]
fn extra_body_bypasses_projection_without_redirecting_the_resource() -> TestResult {
    assert_rows("extra_body_bypasses_projection_without_redirecting_the_resource")
}

#[test]
fn extra_body_cannot_repair_an_earlier_projection_failure() -> TestResult {
    assert_rows("extra_body_cannot_repair_an_earlier_projection_failure")
}

#[test]
fn merged_body_text_preserves_key_positions_and_binary64_numbers() -> TestResult {
    assert_rows("merged_body_text_preserves_key_positions_and_binary64_numbers")
}

#[test]
fn client_snapshots_inputs_and_preparations_do_not_mutate_controls() -> TestResult {
    assert_rows("client_snapshots_inputs_and_preparations_do_not_mutate_controls")
}
