//! Unknown service union members through the public cloud conversation entries.

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
        include_str!("fixtures/cloud_conversation_unknown_unions.json"),
        test,
    )
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_members_reach_every_request_union() -> TestResult {
    check_rows("unknown_members_reach_every_request_union")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn null_known_members_allow_union_fallback() -> TestResult {
    check_rows("null_known_members_allow_union_fallback")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn malformed_known_members_do_not_activate_fallback() -> TestResult {
    check_rows("malformed_known_members_do_not_activate_fallback")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn known_members_suppress_union_fallback() -> TestResult {
    check_rows("known_members_suppress_union_fallback")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selected_unknown_members_reject_invalid_tuples() -> TestResult {
    check_rows("selected_unknown_members_reject_invalid_tuples")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn known_members_ignore_unselected_invalid_fallback() -> TestResult {
    check_rows("known_members_ignore_unselected_invalid_fallback")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn ordinary_records_discard_union_fallback() -> TestResult {
    check_rows("ordinary_records_discard_union_fallback")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_documents_keep_values_and_key_identity() -> TestResult {
    check_rows("unknown_documents_keep_values_and_key_identity")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_proto_name_is_an_ordinary_member() -> TestResult {
    check_rows("unknown_proto_name_is_an_ordinary_member")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn union_lists_keep_order_repeats_and_omit_nulls() -> TestResult {
    check_rows("union_lists_keep_order_repeats_and_omit_nulls")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_document_arrays_are_not_blob_fields() -> TestResult {
    check_rows("unknown_document_arrays_are_not_blob_fields")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn unknown_replacements_reach_raw_and_simple_entries() -> TestResult {
    check_rows("unknown_replacements_reach_raw_and_simple_entries")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn union_errors_keep_hook_and_route_precedence() -> TestResult {
    check_rows("union_errors_keep_hook_and_route_precedence")
}
