//! Controlled summary generation through registered model adapters.
#![cfg(test)]
/// Registered adapter and recorded corpus support.
mod support {
    /// Summary operation consumer.
    pub mod generation;
    /// Numeric-aware recorded value comparison.
    pub mod value;
}
#[test]
fn initial_prompt_preserves_full_request() {
    support::generation::group("initial_prompt_preserves_full_request");
}

#[test]
fn compact_selects_history_or_split_branches() {
    support::generation::group("compact_selects_history_or_split_branches");
}

#[test]
fn maestro_reasoning_summary_receives_selected_level() {
    support::generation::group("maestro_reasoning_summary_receives_selected_level");
}

#[test]
fn maestro_off_summary_omits_reasoning() {
    support::generation::group("maestro_off_summary_omits_reasoning");
}

#[test]
fn maestro_nonreasoning_model_omits_reasoning() {
    support::generation::group("maestro_nonreasoning_model_omits_reasoning");
}

#[test]
fn reasoning_gate_covers_every_level() {
    support::generation::group("reasoning_gate_covers_every_level");
}

#[test]
fn summary_update_and_focus_use_nonempty_text() {
    support::generation::group("summary_update_and_focus_use_nonempty_text");
}

#[test]
fn summary_budgets_floor_without_clamping() {
    support::generation::group("summary_budgets_floor_without_clamping");
}

#[test]
fn returned_text_blocks_keep_order_empty_blocks_and_partial_abort() {
    support::generation::group("returned_text_blocks_keep_order_empty_blocks_and_partial_abort");
}

#[test]
fn history_errors_keep_prefix_and_fallback() {
    support::generation::group("history_errors_keep_prefix_and_fallback");
}

#[test]
fn prefix_errors_keep_distinct_prefix_and_fallback() {
    support::generation::group("prefix_errors_keep_distinct_prefix_and_fallback");
}

#[test]
fn invocation_setup_failure_is_not_reworded() {
    support::generation::group("invocation_setup_failure_is_not_reworded");
}

#[test]
fn raw_projection_reuses_message_transforms_and_selected_fields() {
    support::generation::group("raw_projection_reuses_message_transforms_and_selected_fields");
}

#[test]
fn raw_selected_content_ignores_unknown_blocks_and_unused_fields() {
    support::generation::group("raw_selected_content_ignores_unknown_blocks_and_unused_fields");
}

#[test]
fn reached_invalid_content_fails_before_invocation() {
    support::generation::group("reached_invalid_content_fails_before_invocation");
}

#[test]
fn serialization_keeps_whitespace_tags_and_number_spelling() {
    support::generation::group("serialization_keeps_whitespace_tags_and_number_spelling");
}

#[test]
fn shared_serializer_keeps_existing_tool_limits() {
    support::generation::group("shared_serializer_keeps_existing_tool_limits");
}

#[test]
fn split_requests_forward_same_controls() {
    support::generation::group("split_requests_forward_same_controls");
}

#[test]
fn compact_appends_sorted_disjoint_file_sections() {
    support::generation::group("compact_appends_sorted_disjoint_file_sections");
}

#[test]
fn missing_kept_id_is_checked_after_generation() {
    support::generation::group("missing_kept_id_is_checked_after_generation");
}

#[test]
fn compaction_keeps_supplied_preparation_unchanged() {
    support::generation::group("compaction_keeps_supplied_preparation_unchanged");
}

#[test]
fn raw_summary_path_keeps_scalar_safe_tool_clipping() {
    support::generation::group("raw_summary_path_keeps_scalar_safe_tool_clipping");
}

#[test]
fn split_failure_returns_before_live_sibling_settles() {
    support::generation::lifetime();
}

#[test]
fn maestro_large_history_generates_compaction_result() {
    support::generation::large_history(false);
}

#[test]
fn maestro_applied_summary_rebuilds_smaller_valid_context() {
    support::generation::large_history(true);
}
