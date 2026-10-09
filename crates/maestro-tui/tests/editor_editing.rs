#![cfg(test)]
//! Basic multiline edits through the retained editor.
#[path = "fixtures/editor_support/mod.rs"]
mod support;
#[test]
fn editor_lines_and_cursor_are_independent_snapshots() {
    support::run("editor_lines_and_cursor_are_independent_snapshots");
}
#[test]
fn editor_options_normalize_and_request_on_change() {
    support::run("editor_options_normalize_and_request_on_change");
}
#[test]
fn editor_replacement_normalizes_and_is_undoable() {
    support::run("editor_replacement_normalizes_and_is_undoable");
}
#[test]
fn editor_insert_splices_lines_as_one_undo() {
    support::run("editor_insert_splices_lines_as_one_undo");
}
#[test]
fn editor_horizontal_edits_keep_clusters() {
    support::run("editor_horizontal_edits_keep_clusters");
}
#[test]
fn editor_horizontal_movement_crosses_logical_lines() {
    support::run("editor_horizontal_movement_crosses_logical_lines");
}

#[test]
fn editor_deletion_merges_and_reports_noops() {
    support::run("editor_deletion_merges_and_reports_noops");
}

#[test]
fn editor_undo_coalesces_only_typing_runs() {
    support::run("editor_undo_coalesces_only_typing_runs");
}

#[test]
fn editor_movement_and_newline_break_undo_runs() {
    support::run("editor_movement_and_newline_break_undo_runs");
}

#[test]
fn editor_noop_deletes_do_not_capture() {
    support::run("editor_noop_deletes_do_not_capture");
}

#[test]
fn editor_input_decoding_uses_registered_keys() {
    support::run("editor_input_decoding_uses_registered_keys");
}
#[test]
fn editor_bindings_keep_precedence() {
    support::run("editor_bindings_keep_precedence");
}
#[test]
fn editor_newline_input_classes_split_once() {
    support::run("editor_newline_input_classes_split_once");
}
#[test]
fn editor_backslash_enter_uses_adjacent_character() {
    support::run("editor_backslash_enter_uses_adjacent_character");
}

#[test]
fn editor_rebound_submit_backslash_preserves_order() {
    support::run("editor_rebound_submit_backslash_preserves_order");
}

#[test]
fn editor_disabled_submit_keeps_text_and_undo() {
    support::run("editor_disabled_submit_keeps_text_and_undo");
}

#[test]
fn editor_submit_trims_clears_before_callbacks() {
    support::run("editor_submit_trims_clears_before_callbacks");
}

#[test]
fn editor_typing_chunk_whitespace_is_not_normalized() {
    support::run("editor_typing_chunk_whitespace_is_not_normalized");
}
