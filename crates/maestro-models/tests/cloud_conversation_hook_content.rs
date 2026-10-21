//! Hook-selected service content through the public cloud conversation entries.

#[cfg(not(target_arch = "wasm32"))]
use crate::bedrock_rows as rows;
#[cfg(not(target_arch = "wasm32"))]
use crate::chat;

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;

#[cfg(not(target_arch = "wasm32"))]
/// Check the complete wire observations owned by this behavior.
fn check_rows(test: &str) -> TestResult {
    rows::check_rows(
        include_str!("fixtures/cloud_conversation_hook_content.json"),
        test,
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_tools_keep_structured_results_cache_and_server_fields() -> TestResult {
    check_rows("hook_tools_keep_structured_results_cache_and_server_fields")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_known_fields_omit_absent_and_null_members() -> TestResult {
    check_rows("hook_known_fields_omit_absent_and_null_members")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_known_fields_use_native_type_failures() -> TestResult {
    check_rows("hook_known_fields_use_native_type_failures")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_media_keep_bytes_locations_and_errors() -> TestResult {
    check_rows("hook_media_keep_bytes_locations_and_errors")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_documents_keep_sources_and_citations() -> TestResult {
    check_rows("hook_documents_keep_sources_and_citations")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_records_drop_unselected_members() -> TestResult {
    check_rows("hook_records_drop_unselected_members")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_lists_keep_order_repeats_and_omit_nulls() -> TestResult {
    check_rows("hook_lists_keep_order_repeats_and_omit_nulls")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_open_documents_keep_values_and_key_order() -> TestResult {
    check_rows("hook_open_documents_keep_values_and_key_order")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_search_and_citations_keep_locations_and_order() -> TestResult {
    check_rows("hook_search_and_citations_keep_locations_and_order")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_guard_content_reaches_message_and_system() -> TestResult {
    check_rows("hook_guard_content_reaches_message_and_system")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_reasoning_keeps_redacted_bytes() -> TestResult {
    check_rows("hook_reasoning_keeps_redacted_bytes")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_replacements_reach_raw_and_simple_entries() -> TestResult {
    check_rows("hook_replacements_reach_raw_and_simple_entries")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn hook_errors_and_routes_precede_content_decoding() -> TestResult {
    check_rows("hook_errors_and_routes_precede_content_decoding")
}
