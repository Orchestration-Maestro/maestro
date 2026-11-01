//! Retained tool rows through their public renderer and component interfaces.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "tool_row_support/mod.rs"]
mod tool_row_support;

#[test]
fn renderer_slots_choose_each_override_independently() {
    tool_row_support::group("renderer_slots_choose_each_override_independently");
}

#[test]
fn renderer_slots_ignore_parameter_schema_identity() {
    tool_row_support::group("renderer_slots_ignore_parameter_schema_identity");
}

#[test]
fn built_in_headers_are_not_duplicated() {
    tool_row_support::group("built_in_headers_are_not_duplicated");
}

#[test]
fn read_display_keeps_alias_paths() {
    tool_row_support::group("read_display_keeps_alias_paths");
}

#[test]
fn renderer_slots_share_state_and_keep_last_components() {
    tool_row_support::group("renderer_slots_share_state_and_keep_last_components");
}

#[test]
fn render_context_forwards_every_update_route() {
    tool_row_support::group("render_context_forwards_every_update_route");
    tool_row_support::ownership::live_callback_phases();
}

#[test]
fn renderer_failures_reset_only_the_failed_slot() {
    tool_row_support::group("renderer_failures_reset_only_the_failed_slot");
    tool_row_support::ownership::child_error();
}

#[test]
fn generic_fallback_pretty_prints_only_without_a_definition() {
    tool_row_support::group("generic_fallback_pretty_prints_only_without_a_definition");
}

#[test]
fn shell_and_background_follow_phase_before_error() {
    tool_row_support::group("shell_and_background_follow_phase_before_error");
}

#[test]
fn empty_renderer_output_and_visible_space_styles_are_retained() {
    tool_row_support::group("empty_renderer_output_and_visible_space_styles_are_retained");
}

#[test]
fn built_in_previews_remove_only_trailing_empty_rows() {
    tool_row_support::group("built_in_previews_remove_only_trailing_empty_rows");
}

#[test]
fn compact_reads_expand_to_the_original_content() {
    tool_row_support::group("compact_reads_expand_to_the_original_content");
}

#[test]
fn compact_read_ranges_precede_live_expand_hints() {
    tool_row_support::group("compact_read_ranges_precede_live_expand_hints");
}

#[test]
fn inherited_result_slots_accept_sparse_persisted_details() {
    tool_row_support::group("inherited_result_slots_accept_sparse_persisted_details");
}

#[test]
fn image_support_and_visibility_select_inline_or_text_output() {
    tool_row_support::group("image_support_and_visibility_select_inline_or_text_output");
}

#[test]
fn image_width_defaults_and_setter_use_the_reference_cells() {
    tool_row_support::group("image_width_defaults_and_setter_use_the_reference_cells");
}

#[test]
fn image_conversion_failure_keeps_the_text_shell() {
    tool_row_support::group("image_conversion_failure_keeps_the_text_shell");
}

#[test]
fn renderer_shell_override_inherits_per_definition() {
    tool_row_support::group("renderer_shell_override_inherits_per_definition");
}

#[test]
fn image_constructor_policies_are_consumed_without_a_setter() {
    tool_row_support::group("image_constructor_policies_are_consumed_without_a_setter");
}

#[test]
fn image_blocks_keep_order_and_skip_incomplete_payloads() {
    tool_row_support::group("image_blocks_keep_order_and_skip_incomplete_payloads");
}

#[test]
fn tool_row_outer_children_keep_shared_edits() {
    tool_row_support::group("tool_row_outer_children_keep_shared_edits");
}

#[test]
fn tool_row_invalidation_reads_live_theme() {
    tool_row_support::group("tool_row_invalidation_reads_live_theme");
    tool_row_support::ownership::invalidation_error();
}

#[test]
fn edit_large_preview_settles_without_a_full_redraw() {
    tool_row_support::native::run("edit_large_preview_settles_without_a_full_redraw");
}

#[test]
fn edit_replay_rebuilds_preview_without_complete_arguments() {
    tool_row_support::native::run("edit_replay_rebuilds_preview_without_complete_arguments");
}

#[test]
fn edit_preflight_failure_does_not_render_a_diff() {
    tool_row_support::native::run("edit_preflight_failure_does_not_render_a_diff");
}

#[test]
fn image_replacement_uses_current_payload_and_mime() {
    tool_row_support::ownership::replacement();
}

#[test]
fn image_conversion_leaves_no_deferred_publication() {
    tool_row_support::ownership::no_deferred_publication();
}

#[test]
fn tool_row_callbacks_release_without_retaining_the_writer() {
    tool_row_support::ownership::teardown();
}

#[test]
fn generic_tool_rows_render_each_argument_root() {
    tool_row_support::argument_roots();
}

/// Every fixture group has a public test owner.
const FIXTURE_GROUPS: &[&str] = &[
    "renderer_slots_choose_each_override_independently",
    "renderer_slots_ignore_parameter_schema_identity",
    "built_in_headers_are_not_duplicated",
    "read_display_keeps_alias_paths",
    "renderer_slots_share_state_and_keep_last_components",
    "render_context_forwards_every_update_route",
    "renderer_failures_reset_only_the_failed_slot",
    "generic_fallback_pretty_prints_only_without_a_definition",
    "shell_and_background_follow_phase_before_error",
    "empty_renderer_output_and_visible_space_styles_are_retained",
    "built_in_previews_remove_only_trailing_empty_rows",
    "compact_reads_expand_to_the_original_content",
    "compact_read_ranges_precede_live_expand_hints",
    "inherited_result_slots_accept_sparse_persisted_details",
    "image_support_and_visibility_select_inline_or_text_output",
    "image_width_defaults_and_setter_use_the_reference_cells",
    "image_conversion_failure_keeps_the_text_shell",
    "renderer_shell_override_inherits_per_definition",
    "image_constructor_policies_are_consumed_without_a_setter",
    "image_blocks_keep_order_and_skip_incomplete_payloads",
    "tool_row_outer_children_keep_shared_edits",
    "tool_row_invalidation_reads_live_theme",
];

#[test]
fn typed_result_bindings_prepare_each_rebuild() {
    tool_row_support::ownership::typed_rebuilds();
}

#[test]
fn self_rendered_empty_tool_rows_take_no_rows() {
    tool_row_support::empty_self_rendered();
}

#[test]
fn self_rendered_empty_rows_keep_caller_children() {
    tool_row_support::empty_self_rendered_keeps_caller_children();
}

#[test]
fn self_rendered_rows_walk_live_children() {
    tool_row_support::self_rendered_rows_walk_live_children();
}

#[test]
fn fallback_results_collapse_until_expanded() {
    tool_row_support::collapsed_fallback();
}
