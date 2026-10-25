//! Recorded custom-message and skill-invocation frames through public widget operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "custom_and_skill_support/mod.rs"]
mod custom_and_skill_support;

use custom_and_skill_support::{group, later_render_errors, released_handles, traversal};

#[test]
fn custom_default_strings_keep_markdown_and_panel_rows() {
    group("custom_default_strings_keep_markdown_and_panel_rows");
}

#[test]
fn custom_text_parts_keep_order_duplicates_and_empty_boundaries() {
    group("custom_text_parts_keep_order_duplicates_and_empty_boundaries");
}

#[test]
fn custom_labels_are_literal_bold_and_do_not_filter_display() {
    group("custom_labels_are_literal_bold_and_do_not_filter_display");
}

#[test]
fn message_layout_reuses_padding_and_wrapping() {
    group("message_layout_reuses_padding_and_wrapping");
}

#[test]
fn custom_renderer_outcomes_select_only_one_body() {
    group("custom_renderer_outcomes_select_only_one_body");
}

#[test]
fn custom_expansion_skips_equal_values_and_invalidation_rebuilds() {
    group("custom_expansion_skips_equal_values_and_invalidation_rebuilds");
}

#[test]
fn custom_rebuild_replaces_renderers_and_preserves_outer_edits() {
    group("custom_rebuild_replaces_renderers_and_preserves_outer_edits");
}

#[test]
fn custom_theme_refresh_samples_background_before_foreground_rebuild() {
    group("custom_theme_refresh_samples_background_before_foreground_rebuild");
}

#[test]
fn custom_supplied_markdown_callbacks_are_retained() {
    group("custom_supplied_markdown_callbacks_are_retained");
}

#[test]
fn skill_collapsed_and_expanded_rows_keep_their_own_spacing() {
    group("skill_collapsed_and_expanded_rows_keep_their_own_spacing");
}

#[test]
fn skill_expanded_body_keeps_markdown_and_ignores_other_fields() {
    group("skill_expanded_body_keeps_markdown_and_ignores_other_fields");
}

#[test]
fn skill_hints_read_live_configured_keys_on_each_rebuild() {
    group("skill_hints_read_live_configured_keys_on_each_rebuild");
}

#[test]
fn skill_repeated_expansion_and_invalidation_refresh_current_theme() {
    group("skill_repeated_expansion_and_invalidation_refresh_current_theme");
}

#[test]
fn skill_supplied_markdown_callbacks_and_child_aliases_survive() {
    group("skill_supplied_markdown_callbacks_and_child_aliases_survive");
}

#[test]
fn custom_later_render_errors_propagate_without_default_rebuild() {
    later_render_errors();
}

#[test]
fn custom_handles_release_on_replacement_and_final_drop() {
    released_handles();
}

#[test]
fn custom_text_selection_makes_one_pass_over_recorded_parts() {
    traversal();
}
