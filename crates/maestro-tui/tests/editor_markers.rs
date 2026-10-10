#![cfg(test)]
//! Owned paste markers through the retained editor.
#[path = "fixtures/editor_support/mod.rs"]
mod support;
#[test]
fn markers_distinguish_owned_numeric_and_canonical_identity() {
    support::run("markers_distinguish_owned_numeric_and_canonical_identity");
}
#[test]
fn markers_bound_grapheme_and_word_actions() {
    support::run("markers_bound_grapheme_and_word_actions");
}
#[test]
fn markers_bound_word_runs_on_both_sides() {
    support::run("markers_bound_word_runs_on_both_sides");
}
#[test]
fn markers_preserve_table_across_edit_and_history() {
    support::run("markers_preserve_table_across_edit_and_history");
}
#[test]
fn markers_kill_yank_keeps_payload() {
    support::run("markers_kill_yank_keeps_payload");
}
#[test]
fn markers_expand_in_insertion_order_not_to_fixed_point() {
    support::run("markers_expand_in_insertion_order_not_to_fixed_point");
}
#[test]
fn markers_expand_literal_replacement_characters() {
    support::run("markers_expand_literal_replacement_characters");
}
#[test]
fn markers_submit_expands_trims_and_resets() {
    support::run("markers_submit_expands_trims_and_resets");
}
#[test]
fn markers_render_visually_split_but_edit_atomically() {
    support::run("markers_render_visually_split_but_edit_atomically");
}
#[test]
fn markers_preserve_intersecting_graphemes() {
    support::run("markers_preserve_intersecting_graphemes");
}
#[test]
fn marker_visual_targets_keep_atomic_selection() {
    support::run("marker_visual_targets_keep_atomic_selection");
}
#[test]
fn markers_highlight_owned_span_not_unknown_text() {
    support::run("markers_highlight_owned_span_not_unknown_text");
}
#[test]
fn maestro_editor_creates_a_paste_marker_for_large_pastes() {
    support::run("maestro_editor_creates_a_paste_marker_for_large_pastes");
}
#[test]
fn maestro_editor_treats_paste_marker_as_single_unit_for_right_arrow() {
    support::run("maestro_editor_treats_paste_marker_as_single_unit_for_right_arrow");
}
#[test]
fn maestro_editor_treats_paste_marker_as_single_unit_for_left_arrow() {
    support::run("maestro_editor_treats_paste_marker_as_single_unit_for_left_arrow");
}
#[test]
fn maestro_editor_treats_paste_marker_as_single_unit_for_backspace() {
    support::run("maestro_editor_treats_paste_marker_as_single_unit_for_backspace");
}
#[test]
fn maestro_editor_treats_paste_marker_as_single_unit_for_forward_delete() {
    support::run("maestro_editor_treats_paste_marker_as_single_unit_for_forward_delete");
}
#[test]
fn maestro_editor_treats_paste_marker_as_single_unit_for_word_movement() {
    support::run("maestro_editor_treats_paste_marker_as_single_unit_for_word_movement");
}
#[test]
fn maestro_editor_undo_restores_marker_after_backspace_deletion() {
    support::run("maestro_editor_undo_restores_marker_after_backspace_deletion");
}
#[test]
fn maestro_editor_handles_multiple_paste_markers_in_same_line() {
    support::run("maestro_editor_handles_multiple_paste_markers_in_same_line");
}
#[test]
fn maestro_editor_does_not_treat_manually_typed_marker_like_text_as_atomic_no_valid_paste_id() {
    support::run(
        "maestro_editor_does_not_treat_manually_typed_marker_like_text_as_atomic_no_valid_paste_id",
    );
}
#[test]
fn maestro_editor_does_not_crash_when_paste_marker_is_wider_than_terminal_width() {
    support::run("maestro_editor_does_not_crash_when_paste_marker_is_wider_than_terminal_width");
}
#[test]
fn maestro_editor_does_not_crash_when_text_paste_marker_exceeds_terminal_width_with_cursor_on_marker()
 {
    support::run(
        "maestro_editor_does_not_crash_when_text_paste_marker_exceeds_terminal_width_with_cursor_on_marker",
    );
}
#[test]
fn maestro_editor_wordwrapline_re_checks_overflow_after_backtracking_to_wrap_opportunity() {
    support::run(
        "maestro_editor_wordwrapline_re_checks_overflow_after_backtracking_to_wrap_opportunity",
    );
}
#[test]
fn maestro_editor_expands_large_pasted_content_literally_in_getexpandedtext() {
    support::run("maestro_editor_expands_large_pasted_content_literally_in_getexpandedtext");
}
#[test]
fn maestro_editor_snaps_to_the_paste_marker_start_when_navigating_down_into_it() {
    support::run("maestro_editor_snaps_to_the_paste_marker_start_when_navigating_down_into_it");
}
#[test]
fn maestro_editor_preserves_sticky_column_when_navigating_through_paste_marker_line() {
    support::run(
        "maestro_editor_preserves_sticky_column_when_navigating_through_paste_marker_line",
    );
}
#[test]
fn maestro_editor_does_not_get_stuck_moving_down_from_a_multi_visual_line_paste_marker() {
    support::run(
        "maestro_editor_does_not_get_stuck_moving_down_from_a_multi_visual_line_paste_marker",
    );
}
#[test]
fn maestro_editor_skips_marker_continuation_vls_when_preferred_col_falls_in_marker_tail() {
    support::run(
        "maestro_editor_skips_marker_continuation_vls_when_preferred_col_falls_in_marker_tail",
    );
}
#[test]
fn maestro_editor_submits_large_pasted_content_literally() {
    support::run("maestro_editor_submits_large_pasted_content_literally");
}
