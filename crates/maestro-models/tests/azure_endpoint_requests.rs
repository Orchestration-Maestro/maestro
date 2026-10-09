//! Cloud endpoint request policy through controlled public invocations.
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/azure_endpoint.rs"]
mod endpoint;
#[path = "support/json.rs"]
mod json;
#[test]
fn azure_route_configured_endpoints() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_route_configured_endpoints"),
    )
}

#[test]
fn azure_normalize_url_boundaries() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_normalize_url_boundaries"),
    )
}

#[test]
fn azure_preserve_proxy_queries_in_request_targets() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_preserve_proxy_queries_in_request_targets"),
    )
}

#[test]
fn azure_select_url_sources_without_fallback_after_failure() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_select_url_sources_without_fallback_after_failure"),
    )
}

#[test]
fn azure_select_and_encode_api_version() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_select_and_encode_api_version"),
    )
}

#[test]
fn azure_parse_deployment_map_before_selecting_model() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_parse_deployment_map_before_selecting_model"),
    )
}

#[test]
fn azure_distinguish_raw_and_simple_key_selection() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_distinguish_raw_and_simple_key_selection"),
    )
}

#[test]
fn azure_layer_headers_without_generated_affinity() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_layer_headers_without_generated_affinity"),
    )
}

#[test]
fn azure_trim_scope_headers_with_script_whitespace() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_trim_scope_headers_with_script_whitespace"),
    )
}

#[test]
fn azure_select_raw_reasoning_without_standard_provider_exceptions() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_select_raw_reasoning_without_standard_provider_exceptions"),
    )
}

#[test]
fn azure_map_requested_effort_but_not_summary_default() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_map_requested_effort_but_not_summary_default"),
    )
}

#[test]
fn azure_keep_numeric_omission_rules() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_keep_numeric_omission_rules"),
    )
}

#[test]
fn azure_send_session_key_independently_of_cache_preference() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_send_session_key_independently_of_cache_preference"),
    )
}

#[test]
fn azure_separate_simple_budgets_and_effort_from_raw() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_separate_simple_budgets_and_effort_from_raw"),
    )
}

#[test]
fn azure_bound_simple_defaults_using_model_limits() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_bound_simple_defaults_using_model_limits"),
    )
}

#[test]
fn azure_clamp_simple_effort_with_shared_support() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_clamp_simple_effort_with_shared_support"),
    )
}

#[test]
fn azure_omit_empty_tools_and_preserve_nonempty_order() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_omit_empty_tools_and_preserve_nonempty_order"),
    )
}

#[test]
fn azure_select_all_four_tool_identity_providers() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_select_all_four_tool_identity_providers"),
    )
}

#[test]
fn azure_compose_history_images_unicode_and_missing_results() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_compose_history_images_unicode_and_missing_results"),
    )
}

#[test]
fn azure_preserve_empty_input_classes() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_preserve_empty_input_classes"),
    )
}

#[test]
fn azure_ignore_unadmitted_transport_and_body_fields() -> chat::TestResult {
    chat::block_on(
        false,
        endpoint::assert_rows("azure_ignore_unadmitted_transport_and_body_fields"),
    )
}

#[path = "support/azure_cases.rs"]
mod azure_cases;
#[path = "support/response_output.rs"]
mod response_output;
