//! Hook-selected service content through the public cloud conversation entries.

#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock.rs"]
mod bedrock;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/child_process.rs"]
mod child_process;
use bedrock::json;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock_rows.rs"]
mod rows;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;

use chat::TestResult;

/// Check the complete wire observations owned by this behavior.
fn check_rows(test: &str) -> TestResult {
    rows::check_rows(
        include_str!("fixtures/cloud_conversation_hook_content.json"),
        test,
    )
}

#[test]
fn hook_tools_keep_structured_results_cache_and_server_fields() -> TestResult {
    check_rows("hook_tools_keep_structured_results_cache_and_server_fields")
}

#[test]
fn hook_known_fields_omit_absent_and_null_members() -> TestResult {
    check_rows("hook_known_fields_omit_absent_and_null_members")
}

#[test]
fn hook_known_fields_use_native_type_failures() -> TestResult {
    check_rows("hook_known_fields_use_native_type_failures")
}

#[test]
fn hook_media_keep_bytes_locations_and_errors() -> TestResult {
    check_rows("hook_media_keep_bytes_locations_and_errors")
}

#[test]
fn hook_documents_keep_sources_and_citations() -> TestResult {
    check_rows("hook_documents_keep_sources_and_citations")
}

#[test]
fn hook_records_drop_unselected_members() -> TestResult {
    check_rows("hook_records_drop_unselected_members")
}

#[test]
fn hook_lists_keep_order_repeats_and_omit_nulls() -> TestResult {
    check_rows("hook_lists_keep_order_repeats_and_omit_nulls")
}

#[test]
fn hook_open_documents_keep_values_and_key_order() -> TestResult {
    check_rows("hook_open_documents_keep_values_and_key_order")
}

#[test]
fn hook_search_and_citations_keep_locations_and_order() -> TestResult {
    check_rows("hook_search_and_citations_keep_locations_and_order")
}

#[test]
fn hook_guard_content_reaches_message_and_system() -> TestResult {
    check_rows("hook_guard_content_reaches_message_and_system")
}

#[test]
fn hook_reasoning_keeps_redacted_bytes() -> TestResult {
    check_rows("hook_reasoning_keeps_redacted_bytes")
}

#[test]
fn hook_replacements_reach_raw_and_simple_entries() -> TestResult {
    check_rows("hook_replacements_reach_raw_and_simple_entries")
}

#[test]
fn hook_errors_and_routes_precede_content_decoding() -> TestResult {
    check_rows("hook_errors_and_routes_precede_content_decoding")
}
