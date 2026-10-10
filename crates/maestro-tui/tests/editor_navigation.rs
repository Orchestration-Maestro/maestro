#![cfg(test)]
//! Editor navigation through public operations.
#[path = "fixtures/editor_support/mod.rs"]
mod support;
use maestro_tui::{
    Component, Editor, EditorOptions, autocomplete::CursorPosition, tui::InputHandler,
};

#[test]
fn vertical_edges_select_logical_endpoints() {
    support::run("vertical_edges_select_logical_endpoints");
}

#[test]
fn vertical_decisions_cover_clamped_targets() {
    support::run("vertical_decisions_cover_clamped_targets");
}

#[test]
fn page_navigation_uses_live_rows_and_sticky_columns() {
    support::run("page_navigation_uses_live_rows_and_sticky_columns");
}

#[test]
fn word_runs_use_shared_whitespace_classes() {
    support::run("word_runs_use_shared_whitespace_classes");
}

#[test]
fn jump_accepts_literal_chunks_and_decoded_printables() {
    support::run("jump_accepts_literal_chunks_and_decoded_printables");
}

#[test]
fn navigation_uses_last_rendered_layout_width() {
    support::run("navigation_uses_last_rendered_layout_width");
}

#[test]
fn sticky_resets_follow_actual_cursor_changes() {
    support::run("sticky_resets_follow_actual_cursor_changes");
}

#[test]
fn word_runs_use_exact_punctuation_classes() {
    support::run("word_runs_use_exact_punctuation_classes");
}

#[test]
fn word_endpoints_cross_only_one_logical_line() {
    support::run("word_endpoints_cross_only_one_logical_line");
}

#[test]
fn word_marker_spellings_remain_literal_without_owned_pastes() {
    support::run("word_marker_spellings_remain_literal_without_owned_pastes");
}

#[test]
fn jump_cancellation_falls_through_only_for_controls() {
    support::run("jump_cancellation_falls_through_only_for_controls");
}

#[test]
fn jump_literal_matches_keep_scalar_offsets() {
    support::run("jump_literal_matches_keep_scalar_offsets");
}

#[test]
fn jump_missing_target_breaks_typing_not_sticky() {
    support::run("jump_missing_target_breaks_typing_not_sticky");
}

#[test]
fn navigation_actions_use_live_registered_bindings() {
    support::run("navigation_actions_use_live_registered_bindings");
}

#[test]
fn navigation_bindings_keep_dispatch_order() {
    support::run("navigation_bindings_keep_dispatch_order");
}

#[test]
fn word_bindings_admit_each_default_alias() {
    support::run("word_bindings_admit_each_default_alias");
}

#[test]
fn maestro_editor_deletes_words_correctly_with_ctrl_w_and_alt_backspace() {
    support::run("maestro_editor_deletes_words_correctly_with_ctrl_w_and_alt_backspace");
}

#[test]
fn maestro_editor_navigates_words_correctly_with_ctrl_left_right() {
    support::run("maestro_editor_navigates_words_correctly_with_ctrl_left_right");
}

#[test]
fn maestro_editor_jumps_forward_to_first_occurrence_of_character_on_same_line() {
    support::run("maestro_editor_jumps_forward_to_first_occurrence_of_character_on_same_line");
}

#[test]
fn maestro_editor_jumps_forward_to_next_occurrence_after_cursor() {
    support::run("maestro_editor_jumps_forward_to_next_occurrence_after_cursor");
}

#[test]
fn maestro_editor_jumps_forward_across_multiple_lines() {
    support::run("maestro_editor_jumps_forward_across_multiple_lines");
}

#[test]
fn maestro_editor_jumps_backward_to_first_occurrence_before_cursor_on_same_line() {
    support::run("maestro_editor_jumps_backward_to_first_occurrence_before_cursor_on_same_line");
}

#[test]
fn maestro_editor_jumps_backward_across_multiple_lines() {
    support::run("maestro_editor_jumps_backward_across_multiple_lines");
}

#[test]
fn maestro_editor_does_nothing_when_character_is_not_found_forward() {
    support::run("maestro_editor_does_nothing_when_character_is_not_found_forward");
}

#[test]
fn maestro_editor_does_nothing_when_character_is_not_found_backward() {
    support::run("maestro_editor_does_nothing_when_character_is_not_found_backward");
}

#[test]
fn maestro_editor_is_case_sensitive() {
    support::run("maestro_editor_is_case_sensitive");
}

#[test]
fn maestro_editor_cancels_jump_mode_when_ctrl_is_pressed_again() {
    support::run("maestro_editor_cancels_jump_mode_when_ctrl_is_pressed_again");
}

#[test]
fn maestro_editor_cancels_jump_mode_on_escape_and_processes_the_escape() {
    support::run("maestro_editor_cancels_jump_mode_on_escape_and_processes_the_escape");
}

#[test]
fn maestro_editor_cancels_backward_jump_mode_when_ctrl_alt_is_pressed_again() {
    support::run("maestro_editor_cancels_backward_jump_mode_when_ctrl_alt_is_pressed_again");
}

#[test]
fn maestro_editor_searches_for_special_characters() {
    support::run("maestro_editor_searches_for_special_characters");
}

#[test]
fn maestro_editor_handles_empty_text_gracefully() {
    support::run("maestro_editor_handles_empty_text_gracefully");
}

#[test]
fn maestro_editor_resets_lastaction_when_jumping() {
    support::run("maestro_editor_resets_lastaction_when_jumping");
}

#[test]
fn maestro_editor_preserves_target_column_when_moving_up_through_a_shorter_line() {
    support::run("maestro_editor_preserves_target_column_when_moving_up_through_a_shorter_line");
}

#[test]
fn maestro_editor_preserves_target_column_when_moving_down_through_a_shorter_line() {
    support::run("maestro_editor_preserves_target_column_when_moving_down_through_a_shorter_line");
}

#[test]
fn maestro_editor_resets_sticky_column_on_horizontal_movement_left_arrow() {
    support::run("maestro_editor_resets_sticky_column_on_horizontal_movement_left_arrow");
}

#[test]
fn maestro_editor_resets_sticky_column_on_horizontal_movement_right_arrow() {
    support::run("maestro_editor_resets_sticky_column_on_horizontal_movement_right_arrow");
}

#[test]
fn maestro_editor_resets_sticky_column_on_typing() {
    support::run("maestro_editor_resets_sticky_column_on_typing");
}

#[test]
fn maestro_editor_resets_sticky_column_on_backspace() {
    support::run("maestro_editor_resets_sticky_column_on_backspace");
}

#[test]
fn maestro_editor_resets_sticky_column_on_ctrl_a_move_to_line_start() {
    support::run("maestro_editor_resets_sticky_column_on_ctrl_a_move_to_line_start");
}

#[test]
fn maestro_editor_resets_sticky_column_on_ctrl_e_move_to_line_end() {
    support::run("maestro_editor_resets_sticky_column_on_ctrl_e_move_to_line_end");
}

#[test]
fn maestro_editor_resets_sticky_column_on_word_movement_ctrl_left() {
    support::run("maestro_editor_resets_sticky_column_on_word_movement_ctrl_left");
}

#[test]
fn maestro_editor_resets_sticky_column_on_word_movement_ctrl_right() {
    support::run("maestro_editor_resets_sticky_column_on_word_movement_ctrl_right");
}

#[test]
fn maestro_editor_resets_sticky_column_on_undo() {
    support::run("maestro_editor_resets_sticky_column_on_undo");
}

#[test]
fn maestro_editor_handles_multiple_consecutive_up_down_movements() {
    support::run("maestro_editor_handles_multiple_consecutive_up_down_movements");
}

#[test]
fn maestro_editor_moves_correctly_through_wrapped_visual_lines_without_getting_stuck() {
    support::run(
        "maestro_editor_moves_correctly_through_wrapped_visual_lines_without_getting_stuck",
    );
}

#[test]
fn maestro_editor_handles_settext_resetting_sticky_column() {
    support::run("maestro_editor_handles_settext_resetting_sticky_column");
}

#[test]
fn maestro_editor_sets_preferredvisualcol_when_pressing_right_at_end_of_prompt_last_line() {
    support::run(
        "maestro_editor_sets_preferredvisualcol_when_pressing_right_at_end_of_prompt_last_line",
    );
}

#[test]
fn maestro_editor_handles_editor_resizes_when_preferredvisualcol_is_on_the_same_line() {
    support::run(
        "maestro_editor_handles_editor_resizes_when_preferredvisualcol_is_on_the_same_line",
    );
}

#[test]
fn maestro_editor_handles_editor_resizes_when_preferredvisualcol_is_on_a_different_line() {
    support::run(
        "maestro_editor_handles_editor_resizes_when_preferredvisualcol_is_on_a_different_line",
    );
}

#[test]
fn maestro_editor_rewrapped_lines_target_fits_current_visual_column() {
    support::run("maestro_editor_rewrapped_lines_target_fits_current_visual_column");
}

#[test]
fn maestro_editor_rewrapped_lines_target_shorter_than_current_visual_column() {
    support::run("maestro_editor_rewrapped_lines_target_shorter_than_current_visual_column");
}

#[test]
fn vertical_columns_use_cells_and_visible_atoms() {
    support::run("vertical_columns_use_cells_and_visible_atoms");
}

#[test]
fn vertical_wraps_keep_wide_atoms_reachable() {
    support::run("vertical_wraps_keep_wide_atoms_reachable");
}

#[test]
fn jump_excludes_current_match_at_line_start() {
    support::run("jump_excludes_current_match_at_line_start");
}

#[test]
fn navigation_width_is_saved_before_border_callback() {
    use maestro_tui::{Component, Editor, EditorOptions, tui::InputHandler};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    editor.set_text("abcdefghij\nklmnopqrst");
    let observed = Rc::new(RefCell::new(Vec::new()));
    let owner = editor.clone();
    let output = observed.clone();
    let once = Cell::new(false);
    editor.set_border_color(Rc::new(move |text| {
        if !once.replace(true) {
            owner.handle_input("\x1b[A");
            output.borrow_mut().push(owner.get_cursor());
        }
        text.to_owned()
    }));
    assert_eq!(
        editor.render(5),
        [
            "─────",
            "abcd ",
            "efgh ",
            "ij   ",
            "klmn ",
            "op\x1b[7mq\x1b[0mr ",
            "st   ",
            "─────"
        ]
    );
    assert_eq!(
        *observed.borrow(),
        [maestro_tui::autocomplete::CursorPosition { line: 1, col: 6 }]
    );
    editor.set_border_color(Rc::new(str::to_owned));
}

#[test]
fn vertical_selection_preserves_leading_zero_cell_position() {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    editor.set_text("\u{200b}abc\nx");
    editor.handle_input("\u{1b}[H");
    editor.handle_input("\u{1b}[A");
    editor.insert_text_at_cursor("|");
    assert_eq!(editor.get_text(), "|\u{200b}abc\nx");
}

#[test]
fn vertical_selection_reaches_zero_cell_wrapped_row() {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    editor.set_text("\u{200b}界\nx");
    editor.render(2);
    editor.handle_input("\u{1b}[A");
    assert_eq!(editor.get_cursor(), CursorPosition { line: 0, col: 3 });
    editor.handle_input("\u{1b}[A");
    assert_eq!(editor.get_cursor(), CursorPosition { line: 0, col: 0 });
    editor.insert_text_at_cursor("|");
    assert_eq!(editor.get_text(), "|\u{200b}界\nx");
}

fn up_then_insert(text: &str, keys: &[&str]) -> String {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    editor.set_text(text);
    for key in keys {
        editor.handle_input(key);
    }
    editor.handle_input("\u{1b}[A");
    editor.insert_text_at_cursor("|");
    editor.get_text()
}

#[test]
fn vertical_clamp_to_line_end_lands_after_trailing_zero_cell_atom() {
    assert_eq!(
        up_then_insert("abc\u{200b}\n1234", &[]),
        "abc\u{200b}|\n1234"
    );
}

#[test]
fn vertical_exact_column_stays_before_trailing_zero_cell_atom() {
    assert_eq!(up_then_insert("abc\u{200b}\n123", &[]), "abc|\u{200b}\n123");
}

#[test]
fn vertical_move_onto_zero_width_only_line_follows_requested_column() {
    assert_eq!(
        up_then_insert("\u{200b}\nabcd", &["\u{1b}[H"]),
        "|\u{200b}\nabcd"
    );
    assert_eq!(up_then_insert("\u{200b}\nabcd", &[]), "\u{200b}|\nabcd");
}
