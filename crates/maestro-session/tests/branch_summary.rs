//! Abandoned branch observations through the session library.
#![cfg(test)]
use crate::support;
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_branch_collection_uses_deepest_common_ancestor() {
    support::branch_summary::group("maestro_branch_collection_uses_deepest_common_ancestor");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_collection_reuses_cycle_safe_ancestry() {
    support::branch_summary::group("branch_collection_reuses_cycle_safe_ancestry");
}

#[test]
fn preparation_converts_content_and_skips_metadata() {
    support::branch_summary::prepare_group("preparation_converts_content_and_skips_metadata");
}

#[test]
fn maestro_branch_budget_keeps_summary_exception() {
    support::branch_summary::prepare_group("maestro_branch_budget_keeps_summary_exception");
}

#[test]
fn maestro_branch_files_include_visited_overbudget_call() {
    support::branch_summary::prepare_group("maestro_branch_files_include_visited_overbudget_call");
}

#[test]
fn carried_files_respect_hook_truth_and_array_domains() {
    support::branch_summary::prepare_group("carried_files_respect_hook_truth_and_array_domains");
}

#[test]
fn preparation_preserves_file_set_insertion_order() {
    support::branch_summary::prepare_group("preparation_preserves_file_set_insertion_order");
}

#[test]
fn branch_budget_uses_shared_utf16_estimate() {
    support::branch_summary::prepare_group("branch_budget_uses_shared_utf16_estimate");
}

#[test]
fn reached_branch_fields_fail_only_when_consumed() {
    support::branch_summary::prepare_group("reached_branch_fields_fail_only_when_consumed");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn empty_branch_result_omits_files_and_skips_invocation() {
    support::branch_summary::group("empty_branch_result_omits_files_and_skips_invocation");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn preparation_counts_before_projection() {
    support::branch_summary::group("preparation_counts_before_projection");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_branch_request_preserves_instruction_mode() {
    support::branch_summary::group("maestro_branch_request_preserves_instruction_mode");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_generation_uses_context_and_reserve_defaults() {
    support::branch_summary::group("branch_generation_uses_context_and_reserve_defaults");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_request_reuses_shared_message_projection() {
    support::branch_summary::group("branch_request_reuses_shared_message_projection");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_prompt_keeps_whitespace_and_canonical_arguments() {
    support::branch_summary::group("branch_prompt_keeps_whitespace_and_canonical_arguments");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_controls_have_no_implicit_reasoning_or_retry() {
    support::branch_summary::group("branch_controls_have_no_implicit_reasoning_or_retry");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_branch_summary_distinguishes_abort_error_success() {
    support::branch_summary::group("maestro_branch_summary_distinguishes_abort_error_success");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_projection_error_precedes_model_invocation() {
    support::branch_summary::group("branch_projection_error_precedes_model_invocation");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_success_appends_sorted_disjoint_file_sections() {
    support::branch_summary::group("branch_success_appends_sorted_disjoint_file_sections");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_completion_owns_request_after_inputs_drop() {
    support::branch_summary::lifetime();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn branch_request_goes_through_a_supplied_stream_function() {
    support::branch_summary::stream_function();
}
