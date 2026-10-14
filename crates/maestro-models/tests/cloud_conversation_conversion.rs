//! Bedrock request conversion of history, tools, images, cache points and thinking, observed
//! through the hook input and the body the SDK sent.

use crate::bedrock_rows as rows;
use crate::chat;

use chat::TestResult;

/// Run the rows `test` owns in the conversion fixture.
fn check_rows(test: &str) -> TestResult {
    rows::check_rows(
        include_str!("fixtures/cloud_conversation_conversion.json"),
        test,
    )
}

#[test]
fn history_keeps_user_content_and_order() -> TestResult {
    check_rows("history_keeps_user_content_and_order")
}

#[test]
fn history_skips_blank_assistants_without_trimming_kept_text() -> TestResult {
    check_rows("history_skips_blank_assistants_without_trimming_kept_text")
}

#[test]
fn history_selects_thinking_signature_or_plain_text() -> TestResult {
    check_rows("history_selects_thinking_signature_or_plain_text")
}

#[test]
fn history_uses_shared_projection_and_id_associations() -> TestResult {
    check_rows("history_uses_shared_projection_and_id_associations")
}

#[test]
fn history_groups_only_consecutive_tool_results() -> TestResult {
    check_rows("history_groups_only_consecutive_tool_results")
}

#[test]
fn images_decode_supported_mime_types() -> TestResult {
    check_rows("images_decode_supported_mime_types")
}

#[test]
fn image_failures_precede_hooks_and_later_options() -> TestResult {
    check_rows("image_failures_precede_hooks_and_later_options")
}

#[test]
fn tool_choices_keep_schema_and_omission() -> TestResult {
    check_rows("tool_choices_keep_schema_and_omission")
}

#[test]
fn cache_retention_uses_explicit_then_environment() -> TestResult {
    check_rows("cache_retention_uses_explicit_then_environment")
}

#[test]
fn cache_support_uses_identity_name_and_force_gate() -> TestResult {
    check_rows("cache_support_uses_identity_name_and_force_gate")
}

#[test]
fn cache_points_follow_system_and_final_user_only() -> TestResult {
    check_rows("cache_points_follow_system_and_final_user_only")
}

#[test]
fn thinking_selects_adaptive_or_budget_fields() -> TestResult {
    check_rows("thinking_selects_adaptive_or_budget_fields")
}

#[test]
fn thinking_effort_preserves_native_xhigh_and_open_mapping() -> TestResult {
    check_rows("thinking_effort_preserves_native_xhigh_and_open_mapping")
}

#[test]
fn thinking_budgets_and_interleaving_keep_independent_overrides() -> TestResult {
    check_rows("thinking_budgets_and_interleaving_keep_independent_overrides")
}

#[test]
fn government_targets_omit_only_thinking_display() -> TestResult {
    check_rows("government_targets_omit_only_thinking_display")
}

#[test]
fn simple_entry_forwards_common_options_and_adjusts_fixed_budgets() -> TestResult {
    check_rows("simple_entry_forwards_common_options_and_adjusts_fixed_budgets")
}

#[test]
fn payload_hook_changes_delivered_variants_before_wire_encoding() -> TestResult {
    check_rows("payload_hook_changes_delivered_variants_before_wire_encoding")
}

#[test]
fn wire_omits_only_record_nulls_and_preserves_document_nulls() -> TestResult {
    check_rows("wire_omits_only_record_nulls_and_preserves_document_nulls")
}

#[test]
fn delivered_hook_fields_keep_their_declared_domains() -> TestResult {
    check_rows("delivered_hook_fields_keep_their_declared_domains")
}

#[test]
fn conversion_and_hook_errors_keep_source_precedence() -> TestResult {
    check_rows("conversion_and_hook_errors_keep_source_precedence")
}
