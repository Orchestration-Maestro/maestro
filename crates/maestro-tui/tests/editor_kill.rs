#![cfg(test)]
//! Editor kill through public operations.
use crate::editor_support as support;

#[test]
fn kill_boundaries_keep_noop_undo_and_chain_semantics() {
    support::run("kill_boundaries_keep_noop_undo_and_chain_semantics");
}

#[test]
fn kill_newlines_accumulate_in_their_direction() {
    support::run("kill_newlines_accumulate_in_their_direction");
}

#[test]
fn kill_and_yank_chains_follow_action_precedence() {
    support::run("kill_and_yank_chains_follow_action_precedence");
}

#[test]
fn yank_pop_preserves_prefix_suffix_and_undo() {
    support::run("yank_pop_preserves_prefix_suffix_and_undo");
}

#[test]
fn yank_keeps_raw_history_text() {
    support::run("yank_keeps_raw_history_text");
}

#[test]
fn maestro_editor_ctrl_w_then_ctrl_y_returns_the_removed_word() {
    support::run("maestro_editor_ctrl_w_then_ctrl_y_returns_the_removed_word");
}

#[test]
fn maestro_editor_ctrl_u_puts_the_removed_text_in_the_kill_ring() {
    support::run("maestro_editor_ctrl_u_puts_the_removed_text_in_the_kill_ring");
}

#[test]
fn maestro_editor_ctrl_k_puts_the_removed_text_in_the_kill_ring() {
    support::run("maestro_editor_ctrl_k_puts_the_removed_text_in_the_kill_ring");
}

#[test]
fn maestro_editor_ctrl_y_with_an_empty_kill_ring_changes_nothing() {
    support::run("maestro_editor_ctrl_y_with_an_empty_kill_ring_changes_nothing");
}

#[test]
fn maestro_editor_alt_y_after_yank_walks_the_kill_ring() {
    support::run("maestro_editor_alt_y_after_yank_walks_the_kill_ring");
}

#[test]
fn maestro_editor_alt_y_without_a_yank_changes_nothing() {
    support::run("maestro_editor_alt_y_without_a_yank_changes_nothing");
}

#[test]
fn maestro_editor_alt_y_with_one_entry_changes_nothing() {
    support::run("maestro_editor_alt_y_with_one_entry_changes_nothing");
}

#[test]
fn maestro_editor_repeated_ctrl_w_builds_one_kill_entry() {
    support::run("maestro_editor_repeated_ctrl_w_builds_one_kill_entry");
}

#[test]
fn maestro_editor_ctrl_u_collects_multiline_removals_with_newlines() {
    support::run("maestro_editor_ctrl_u_collects_multiline_removals_with_newlines");
}

#[test]
fn maestro_editor_accumulated_kills_prepend_backward_and_append_forward_text() {
    support::run("maestro_editor_accumulated_kills_prepend_backward_and_append_forward_text");
}

#[test]
fn maestro_editor_a_non_delete_action_ends_kill_accumulation() {
    support::run("maestro_editor_a_non_delete_action_ends_kill_accumulation");
}

#[test]
fn maestro_editor_a_non_yank_action_ends_the_alt_y_chain() {
    support::run("maestro_editor_a_non_yank_action_ends_the_alt_y_chain");
}

#[test]
fn maestro_editor_kill_ring_rotation_survives_cycling() {
    support::run("maestro_editor_kill_ring_rotation_survives_cycling");
}

#[test]
fn maestro_editor_deletes_over_line_boundaries_merge_into_one_entry() {
    support::run("maestro_editor_deletes_over_line_boundaries_merge_into_one_entry");
}

#[test]
fn maestro_editor_ctrl_k_at_line_end_joins_lines_and_coalesces() {
    support::run("maestro_editor_ctrl_k_at_line_end_joins_lines_and_coalesces");
}

#[test]
fn maestro_editor_yank_inserts_in_the_middle_of_the_text() {
    support::run("maestro_editor_yank_inserts_in_the_middle_of_the_text");
}

#[test]
fn maestro_editor_yank_pop_replaces_text_in_the_middle() {
    support::run("maestro_editor_yank_pop_replaces_text_in_the_middle");
}

#[test]
fn maestro_editor_multiline_yank_and_pop_mid_text() {
    support::run("maestro_editor_multiline_yank_and_pop_mid_text");
}

#[test]
fn maestro_editor_alt_d_drops_the_next_word_in_the_kill_ring() {
    support::run("maestro_editor_alt_d_drops_the_next_word_in_the_kill_ring");
}

#[test]
fn maestro_editor_alt_d_at_line_end_joins_the_lines() {
    support::run("maestro_editor_alt_d_at_line_end_joins_the_lines");
}

#[test]
fn maestro_editor_one_undo_restores_text_after_ctrl_w() {
    support::run("maestro_editor_one_undo_restores_text_after_ctrl_w");
}

#[test]
fn maestro_editor_one_undo_restores_text_after_ctrl_k() {
    support::run("maestro_editor_one_undo_restores_text_after_ctrl_k");
}

#[test]
fn maestro_editor_one_undo_restores_text_after_ctrl_u() {
    support::run("maestro_editor_one_undo_restores_text_after_ctrl_u");
}

#[test]
fn maestro_editor_one_undo_reverts_a_yank() {
    support::run("maestro_editor_one_undo_reverts_a_yank");
}

#[test]
fn maestro_editor_deletes_that_change_nothing_add_no_undo_step() {
    support::run("maestro_editor_deletes_that_change_nothing_add_no_undo_step");
}

fn ring_editor(entries: &[&str]) -> (maestro_tui::TUI, std::rc::Rc<maestro_tui::Editor>) {
    use maestro_tui::{Editor, EditorOptions, InputHandler};
    let (tui, _, _) = support::host(24);
    let editor = std::rc::Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    for entry in entries {
        editor.set_text(entry);
        editor.handle_input("\x15");
    }
    editor.set_text("");
    (tui, editor)
}
#[test]
fn yank_callbacks_keep_deletion_before_rotation() {
    use maestro_tui::InputHandler;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let _guard = support::globals();
    for nested in [false, true] {
        let (_tui, editor) = ring_editor(&["one", "two"]);
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let owner = editor.clone();
        let once = Cell::new(false);
        editor.set_on_change(Some(Rc::new(move |text| {
            output
                .borrow_mut()
                .push((text.to_owned(), owner.get_cursor().col));
            if nested && !once.replace(true) {
                owner.handle_input("\x1by");
                assert_eq!(owner.get_text(), "two");
            }
        })));
        editor.handle_input("\x19");
        if nested {
            assert_eq!(*events.borrow(), [("two".into(), 3)]);
        } else {
            editor.handle_input("\x1by");
            assert_eq!(
                *events.borrow(),
                [("two".into(), 3), (String::new(), 0), ("one".into(), 3)]
            );
        }
        editor.set_on_change(None);
    }
}

#[test]
fn yank_callbacks_read_live_ring_after_deletion() {
    use maestro_tui::InputHandler;
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let _guard = support::globals();
    let (_tui, editor) = ring_editor(&["A", "B"]);
    editor.handle_input("\x19");
    let events = Rc::new(RefCell::new(Vec::new()));
    let output = events.clone();
    let owner = editor.clone();
    let once = Cell::new(false);
    editor.set_on_change(Some(Rc::new(move |text| {
        output.borrow_mut().push(text.to_owned());
        if !once.replace(true) {
            owner.set_text("C");
            owner.handle_input("\x17");
        }
    })));
    editor.handle_input("\x1by");
    assert_eq!(*events.borrow(), ["", "C", "", "B"]);
    assert_eq!(editor.get_text(), "B");
    assert_eq!(editor.get_cursor().col, 1);
    editor.set_on_change(None);
}

#[test]
fn yank_reentry_never_erases_replacement_text() {
    use maestro_tui::{CursorPosition, InputHandler};
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };
    let _guard = support::globals();
    for mode in ["initial", "deletion", "final"] {
        let (_tui, editor) = ring_editor(&["old", "newer"]);
        if mode != "initial" {
            editor.handle_input("\x19");
        }
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let owner = editor.clone();
        let once = Cell::new(false);
        editor.set_on_change(Some(Rc::new(move |text| {
            output.borrow_mut().push(text.to_owned());
            if (mode != "final" || text == "old") && !once.replace(true) {
                owner.set_text("Q");
            }
        })));
        editor.handle_input(if mode == "initial" { "\x19" } else { "\x1by" });
        assert_eq!(
            editor.get_text(),
            if mode == "deletion" { "Qold" } else { "Q" }
        );
        events.borrow_mut().clear();
        editor.handle_input("\x1by");
        if mode == "deletion" {
            assert_eq!(*events.borrow(), ["Q", "Qnewer"]);
            assert_eq!(editor.get_text(), "Qnewer");
            assert_eq!(editor.get_cursor(), CursorPosition { line: 0, col: 6 });
        } else {
            assert!(events.borrow().is_empty());
            assert_eq!(editor.get_text(), "Q");
            assert_eq!(editor.get_cursor(), CursorPosition { line: 0, col: 1 });
        }
        editor.set_on_change(None);
    }
}

#[test]
fn yank_pop_from_deletion_callback_does_not_rotate_again() {
    use maestro_tui::{CursorPosition, InputHandler};
    use std::{cell::Cell, rc::Rc};
    let _guard = support::globals();
    for (entries, cursor) in [
        (["one", "two"], CursorPosition { line: 0, col: 3 }),
        (["a\nb", "c\nd"], CursorPosition { line: 1, col: 1 }),
    ] {
        let (_tui, editor) = ring_editor(&[]);
        for entry in entries {
            editor.set_text(entry);
            for _ in 0..3 {
                editor.handle_input("\x15");
            }
            editor.set_text("");
        }
        editor.handle_input("\x19");
        let once = Cell::new(false);
        let owner = editor.clone();
        editor.set_on_change(Some(Rc::new(move |_| {
            if !once.replace(true) {
                owner.handle_input("\x1by");
            }
        })));
        editor.handle_input("\x1by");
        editor.set_on_change(None);
        assert_eq!(editor.get_text(), entries[0]);
        assert_eq!(editor.get_cursor(), cursor);
    }
}
