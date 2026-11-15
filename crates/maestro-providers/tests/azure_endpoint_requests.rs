//! Cloud endpoint request policy through controlled public invocations.
#[cfg(not(target_arch = "wasm32"))]
use crate::azure_endpoint as endpoint;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_route_configured_endpoints() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows("azure_route_configured_endpoints"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_normalize_url_boundaries() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows("azure_normalize_url_boundaries"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_preserve_proxy_queries_in_request_targets() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_preserve_proxy_queries_in_request_targets",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_select_url_sources_without_fallback_after_failure() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_select_url_sources_without_fallback_after_failure",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_select_and_encode_api_version() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows("azure_select_and_encode_api_version"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_parse_deployment_map_before_selecting_model() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_parse_deployment_map_before_selecting_model",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_distinguish_raw_and_simple_key_selection() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_distinguish_raw_and_simple_key_selection",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_layer_headers_without_generated_affinity() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_layer_headers_without_generated_affinity",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_trim_scope_headers_with_script_whitespace() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_trim_scope_headers_with_script_whitespace",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_select_raw_reasoning_without_standard_provider_exceptions() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_select_raw_reasoning_without_standard_provider_exceptions",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_map_requested_effort_but_not_summary_default() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_map_requested_effort_but_not_summary_default",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_keep_numeric_omission_rules() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows("azure_keep_numeric_omission_rules"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_send_session_key_independently_of_cache_preference() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_send_session_key_independently_of_cache_preference",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_separate_simple_budgets_and_effort_from_raw() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_separate_simple_budgets_and_effort_from_raw",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_bound_simple_defaults_using_model_limits() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_bound_simple_defaults_using_model_limits",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_clamp_simple_effort_with_shared_support() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_clamp_simple_effort_with_shared_support",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_omit_empty_tools_and_preserve_nonempty_order() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_omit_empty_tools_and_preserve_nonempty_order",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_select_all_four_tool_identity_providers() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_select_all_four_tool_identity_providers",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_compose_history_images_unicode_and_missing_results() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_compose_history_images_unicode_and_missing_results",
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_preserve_empty_input_classes() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows("azure_preserve_empty_input_classes"))
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn azure_ignore_unadmitted_transport_and_body_fields() -> chat::TestResult {
    crate::runtime::run(endpoint::assert_rows(
        "azure_ignore_unadmitted_transport_and_body_fields",
    ))
}
