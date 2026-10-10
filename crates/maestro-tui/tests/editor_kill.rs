#![cfg(test)]
//! Editor kill through public operations.
#[path = "fixtures/editor_support/mod.rs"]
mod support;

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
fn maestro_editor_ctrl_w_saves_deleted_text_to_kill_ring_and_ctrl_y_yanks_it() {
    support::run("maestro_editor_ctrl_w_saves_deleted_text_to_kill_ring_and_ctrl_y_yanks_it");
}

#[test]
fn maestro_editor_ctrl_u_saves_deleted_text_to_kill_ring() {
    support::run("maestro_editor_ctrl_u_saves_deleted_text_to_kill_ring");
}

#[test]
fn maestro_editor_ctrl_k_saves_deleted_text_to_kill_ring() {
    support::run("maestro_editor_ctrl_k_saves_deleted_text_to_kill_ring");
}

#[test]
fn maestro_editor_ctrl_y_does_nothing_when_kill_ring_is_empty() {
    support::run("maestro_editor_ctrl_y_does_nothing_when_kill_ring_is_empty");
}

#[test]
fn maestro_editor_alt_y_cycles_through_kill_ring_after_ctrl_y() {
    support::run("maestro_editor_alt_y_cycles_through_kill_ring_after_ctrl_y");
}

#[test]
fn maestro_editor_alt_y_does_nothing_if_not_preceded_by_yank() {
    support::run("maestro_editor_alt_y_does_nothing_if_not_preceded_by_yank");
}

#[test]
fn maestro_editor_alt_y_does_nothing_if_kill_ring_has_1_entry() {
    support::run("maestro_editor_alt_y_does_nothing_if_kill_ring_has_1_entry");
}

#[test]
fn maestro_editor_consecutive_ctrl_w_accumulates_into_one_kill_ring_entry() {
    support::run("maestro_editor_consecutive_ctrl_w_accumulates_into_one_kill_ring_entry");
}

#[test]
fn maestro_editor_ctrl_u_accumulates_multiline_deletes_including_newlines() {
    support::run("maestro_editor_ctrl_u_accumulates_multiline_deletes_including_newlines");
}

#[test]
fn maestro_editor_backward_deletions_prepend_forward_deletions_append_during_accumulation() {
    support::run(
        "maestro_editor_backward_deletions_prepend_forward_deletions_append_during_accumulation",
    );
}

#[test]
fn maestro_editor_non_delete_actions_break_kill_accumulation() {
    support::run("maestro_editor_non_delete_actions_break_kill_accumulation");
}

#[test]
fn maestro_editor_non_yank_actions_break_alt_y_chain() {
    support::run("maestro_editor_non_yank_actions_break_alt_y_chain");
}

#[test]
fn maestro_editor_kill_ring_rotation_persists_after_cycling() {
    support::run("maestro_editor_kill_ring_rotation_persists_after_cycling");
}

#[test]
fn maestro_editor_consecutive_deletions_across_lines_coalesce_into_one_entry() {
    support::run("maestro_editor_consecutive_deletions_across_lines_coalesce_into_one_entry");
}

#[test]
fn maestro_editor_ctrl_k_at_line_end_deletes_newline_and_coalesces() {
    support::run("maestro_editor_ctrl_k_at_line_end_deletes_newline_and_coalesces");
}

#[test]
fn maestro_editor_handles_yank_in_middle_of_text() {
    support::run("maestro_editor_handles_yank_in_middle_of_text");
}

#[test]
fn maestro_editor_handles_yank_pop_in_middle_of_text() {
    support::run("maestro_editor_handles_yank_pop_in_middle_of_text");
}

#[test]
fn maestro_editor_multiline_yank_and_yank_pop_in_middle_of_text() {
    support::run("maestro_editor_multiline_yank_and_yank_pop_in_middle_of_text");
}

#[test]
fn maestro_editor_alt_d_deletes_word_forward_and_saves_to_kill_ring() {
    support::run("maestro_editor_alt_d_deletes_word_forward_and_saves_to_kill_ring");
}

#[test]
fn maestro_editor_alt_d_at_end_of_line_deletes_newline() {
    support::run("maestro_editor_alt_d_at_end_of_line_deletes_newline");
}

#[test]
fn maestro_editor_undoes_ctrl_w_delete_word_backward() {
    support::run("maestro_editor_undoes_ctrl_w_delete_word_backward");
}

#[test]
fn maestro_editor_undoes_ctrl_k_delete_to_line_end() {
    support::run("maestro_editor_undoes_ctrl_k_delete_to_line_end");
}

#[test]
fn maestro_editor_undoes_ctrl_u_delete_to_line_start() {
    support::run("maestro_editor_undoes_ctrl_u_delete_to_line_start");
}

#[test]
fn maestro_editor_undoes_yank() {
    support::run("maestro_editor_undoes_yank");
}

#[test]
fn maestro_editor_no_op_delete_operations_do_not_push_undo_snapshots() {
    support::run("maestro_editor_no_op_delete_operations_do_not_push_undo_snapshots");
}

fn ring_editor(entries: &[&str]) -> (maestro_tui::TUI, std::rc::Rc<maestro_tui::Editor>) {
    use maestro_tui::{Editor, EditorOptions, tui::InputHandler};
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
    use maestro_tui::tui::InputHandler;
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
    use maestro_tui::tui::InputHandler;
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
    use maestro_tui::{autocomplete::CursorPosition, tui::InputHandler};
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
    use maestro_tui::{autocomplete::CursorPosition, tui::InputHandler};
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
