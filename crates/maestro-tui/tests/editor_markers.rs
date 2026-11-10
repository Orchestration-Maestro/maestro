#![cfg(test)]
//! Owned paste markers through the retained editor.
use crate::editor_support as support;
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
fn maestro_editor_large_paste_becomes_a_marker() {
    support::run("maestro_editor_large_paste_becomes_a_marker");
}
#[test]
fn maestro_editor_right_arrow_steps_over_a_whole_marker() {
    support::run("maestro_editor_right_arrow_steps_over_a_whole_marker");
}
#[test]
fn maestro_editor_left_arrow_steps_over_a_whole_marker() {
    support::run("maestro_editor_left_arrow_steps_over_a_whole_marker");
}
#[test]
fn maestro_editor_backspace_removes_a_whole_marker() {
    support::run("maestro_editor_backspace_removes_a_whole_marker");
}
#[test]
fn maestro_editor_forward_delete_removes_a_whole_marker() {
    support::run("maestro_editor_forward_delete_removes_a_whole_marker");
}
#[test]
fn maestro_editor_word_move_steps_over_a_whole_marker() {
    support::run("maestro_editor_word_move_steps_over_a_whole_marker");
}
#[test]
fn maestro_editor_undo_brings_back_the_marker_a_backspace_removed() {
    support::run("maestro_editor_undo_brings_back_the_marker_a_backspace_removed");
}
#[test]
fn maestro_editor_several_markers_share_one_line() {
    support::run("maestro_editor_several_markers_share_one_line");
}
#[test]
fn maestro_editor_typed_marker_text_without_a_valid_id_is_not_atomic() {
    support::run("maestro_editor_typed_marker_text_without_a_valid_id_is_not_atomic");
}
#[test]
fn maestro_editor_survives_a_marker_wider_than_the_terminal() {
    support::run("maestro_editor_survives_a_marker_wider_than_the_terminal");
}
#[test]
fn maestro_editor_survives_an_overlong_marker_under_the_cursor() {
    support::run("maestro_editor_survives_an_overlong_marker_under_the_cursor");
}
#[test]
fn maestro_editor_wrapping_re_checks_overflow_after_a_backtrack() {
    support::run("maestro_editor_wrapping_re_checks_overflow_after_a_backtrack");
}
#[test]
fn maestro_editor_expanded_text_returns_pasted_content_verbatim() {
    support::run("maestro_editor_expanded_text_returns_pasted_content_verbatim");
}
#[test]
fn maestro_editor_moving_down_into_a_marker_snaps_to_its_start() {
    support::run("maestro_editor_moving_down_into_a_marker_snaps_to_its_start");
}
#[test]
fn maestro_editor_sticky_column_holds_across_a_marker_line() {
    support::run("maestro_editor_sticky_column_holds_across_a_marker_line");
}
#[test]
fn maestro_editor_moving_down_from_a_multi_row_marker_keeps_progress() {
    support::run("maestro_editor_moving_down_from_a_multi_row_marker_keeps_progress");
}
#[test]
fn maestro_editor_a_target_inside_the_marker_tail_skips_its_rows() {
    support::run("maestro_editor_a_target_inside_the_marker_tail_skips_its_rows");
}
#[test]
fn maestro_editor_submits_large_pasted_content_literally() {
    support::run("maestro_editor_submits_large_pasted_content_literally");
}
#[test]
fn marker_snap_column_survives_resize() {
    support::run("marker_snap_column_survives_resize");
}
#[test]
fn markers_jump_and_edit_keep_separate_behavior() {
    support::run("markers_jump_and_edit_keep_separate_behavior");
}
