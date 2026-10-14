//! Public borrowed tree rendering.
#![cfg(test)]
mod tree_render_support;
/// Empty filters render their position and badge.
#[test]
fn empty_view_reports_filter_status() {
    tree_render_support::group("empty_view_reports_filter_status");
}
/// Single-child paths keep their original column.
#[test]
fn single_chains_stay_flat() {
    tree_render_support::group("single_chains_stay_flat");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn branch_gutters_align_after_first_generation() {
    tree_render_support::group("branch_gutters_align_after_first_generation");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn multiple_roots_hide_virtual_connectors() {
    tree_render_support::group("multiple_roots_hide_virtual_connectors");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn active_path_is_independent_of_selection() {
    tree_render_support::group("active_path_is_independent_of_selection");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn filtering_reparents_rows_and_rebuilds_gutters() {
    tree_render_support::group("filtering_reparents_rows_and_rebuilds_gutters");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn foldable_branches_show_expanded_sign() {
    tree_render_support::group("foldable_branches_show_expanded_sign");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn repeated_id_rows_are_not_deduplicated() {
    tree_render_support::group("repeated_id_rows_are_not_deduplicated");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn visible_window_centers_and_clamps() {
    tree_render_support::group("visible_window_centers_and_clamps");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn entry_text_normalizes_only_newline_tab_and_trim() {
    tree_render_support::group("entry_text_normalizes_only_newline_tab_and_trim");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn content_prefix_stops_at_two_hundred_units() {
    tree_render_support::group("content_prefix_stops_at_two_hundred_units");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn assistant_fallback_precedence_and_error_prefix() {
    tree_render_support::group("assistant_fallback_precedence_and_error_prefix");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn shell_and_open_roles_keep_their_labels() {
    tree_render_support::group("shell_and_open_roles_keep_their_labels");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn custom_messages_join_text_without_preview_cap() {
    tree_render_support::group("custom_messages_join_text_without_preview_cap");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn compaction_rounds_thousands_at_half_boundary() {
    tree_render_support::group("compaction_rounds_thousands_at_half_boundary");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn metadata_rows_keep_kind_specific_text() {
    tree_render_support::group("metadata_rows_keep_kind_specific_text");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn attached_labels_precede_entry_content() {
    tree_render_support::group("attached_labels_precede_entry_content");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn shell_call_preview_uses_raw_length_for_ellipsis() {
    tree_render_support::group("shell_call_preview_uses_raw_length_for_ellipsis");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn built_in_calls_keep_argument_precedence_and_defaults() {
    tree_render_support::group("built_in_calls_keep_argument_precedence_and_defaults");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn read_ranges_distinguish_missing_null_and_zero() {
    tree_render_support::group("read_ranges_distinguish_missing_null_and_zero");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn tool_paths_use_live_home_prefix() {
    tree_render_support::group("tool_paths_use_live_home_prefix");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn custom_calls_use_canonical_json_and_forty_unit_prefix() {
    tree_render_support::group("custom_calls_use_canonical_json_and_forty_unit_prefix");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn unmatched_tool_results_keep_fallback_name() {
    tree_render_support::group("unmatched_tool_results_keep_fallback_name");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn tool_lookup_keeps_last_flattened_call() {
    tree_render_support::group("tool_lookup_keeps_last_flattened_call");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn final_lines_fit_cell_width_without_splitting_clusters() {
    tree_render_support::group("final_lines_fit_cell_width_without_splitting_clusters");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn render_observes_theme_changes_and_invalidation() {
    tree_render_support::group("render_observes_theme_changes_and_invalidation");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn malformed_fields_fail_only_when_displayed() {
    tree_render_support::group("malformed_fields_fail_only_when_displayed");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn active_ancestry_self_cycle_terminates() {
    tree_render_support::group("active_ancestry_self_cycle_terminates");
}

/// Replays exact attributed frames through the public renderer.
#[test]
fn hidden_ancestry_cycle_becomes_visible_root() {
    tree_render_support::group("hidden_ancestry_cycle_becomes_visible_root");
}

/// Nonstring tool operands display with field-specific truthy defaults.
#[test]
fn tool_argument_values_render_with_truthy_defaults() {
    tree_render_support::group("tool_argument_values_render_with_truthy_defaults");
}

/// Renaming a corpus case cannot admit a repeated complete input.
#[test]
#[should_panic(expected = "duplicate complete input")]
fn renamed_duplicate_inputs_are_rejected() {
    tree_render_support::reject_renamed_duplicate();
}
