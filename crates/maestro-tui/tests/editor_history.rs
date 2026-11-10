#![cfg(test)]
//! Prompt history through the editor interface.
use crate::editor_support as support;
#[test]
fn history_admission_trims_only_endpoints() {
    support::run("history_admission_trims_only_endpoints");
}

#[test]
fn history_actions_exit_at_their_consuming_branch() {
    support::run("history_actions_exit_at_their_consuming_branch");
}

#[test]
fn history_additions_keep_browse_index_and_order() {
    support::run("history_additions_keep_browse_index_and_order");
}

#[test]
fn history_undo_restores_original_cursor_once() {
    support::run("history_undo_restores_original_cursor_once");
}

#[test]
fn history_submit_keeps_history_and_ring() {
    support::run("history_submit_keeps_history_and_ring");
}

#[test]
fn maestro_editor_up_arrow_with_an_empty_history_changes_nothing() {
    support::run("maestro_editor_up_arrow_with_an_empty_history_changes_nothing");
}

#[test]
fn maestro_editor_up_arrow_on_an_empty_editor_shows_the_newest_entry() {
    support::run("maestro_editor_up_arrow_on_an_empty_editor_shows_the_newest_entry");
}

#[test]
fn maestro_editor_repeated_up_arrow_walks_back_through_history() {
    support::run("maestro_editor_repeated_up_arrow_walks_back_through_history");
}

#[test]
fn maestro_editor_restores_the_draft_on_down_arrow_after_browsing_history() {
    support::run("maestro_editor_restores_the_draft_on_down_arrow_after_browsing_history");
}

#[test]
fn maestro_editor_down_arrow_walks_forward_through_history() {
    support::run("maestro_editor_down_arrow_walks_forward_through_history");
}

#[test]
fn maestro_editor_typing_a_character_leaves_history_mode() {
    support::run("maestro_editor_typing_a_character_leaves_history_mode");
}

#[test]
fn maestro_editor_exits_history_mode_on_settext() {
    support::run("maestro_editor_exits_history_mode_on_settext");
}

#[test]
fn maestro_editor_history_skips_empty_entries() {
    support::run("maestro_editor_history_skips_empty_entries");
}

#[test]
fn maestro_editor_history_skips_a_repeat_of_the_newest_entry() {
    support::run("maestro_editor_history_skips_a_repeat_of_the_newest_entry");
}

#[test]
fn maestro_editor_history_keeps_repeats_that_are_not_adjacent() {
    support::run("maestro_editor_history_keeps_repeats_that_are_not_adjacent");
}

#[test]
fn maestro_editor_a_non_empty_editor_moves_the_cursor_not_history() {
    support::run("maestro_editor_a_non_empty_editor_moves_the_cursor_not_history");
}

#[test]
fn maestro_editor_limits_history_to_100_entries() {
    support::run("maestro_editor_limits_history_to_100_entries");
}

#[test]
fn maestro_editor_places_cursor_at_start_after_browsing_history_upward() {
    support::run("maestro_editor_places_cursor_at_start_after_browsing_history_upward");
}

#[test]
fn maestro_editor_places_cursor_at_end_after_browsing_history_downward() {
    support::run("maestro_editor_places_cursor_at_end_after_browsing_history_downward");
}

#[test]
fn maestro_editor_history_entry_moves_the_cursor_both_ways() {
    support::run("maestro_editor_history_entry_moves_the_cursor_both_ways");
}

#[test]
fn maestro_editor_undo_leaves_history_browsing() {
    support::run("maestro_editor_undo_leaves_history_browsing");
}

#[test]
fn maestro_editor_undo_returns_to_the_state_before_any_history_step() {
    support::run("maestro_editor_undo_returns_to_the_state_before_any_history_step");
}

#[test]
fn history_callbacks_observe_committed_selection() {
    use maestro_tui::{Editor, EditorOptions, interface::InputHandler};
    use std::{cell::RefCell, rc::Rc};
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    editor.add_to_history("old");
    editor.add_to_history("new");
    let events = Rc::new(RefCell::new(Vec::new()));
    let owner = editor.clone();
    let observed = events.clone();
    editor.set_on_change(Some(Rc::new(move |text| {
        observed
            .borrow_mut()
            .push(("first", text.to_owned(), owner.get_cursor().col));
        let next = observed.clone();
        let view = owner.clone();
        owner.set_on_change(Some(Rc::new(move |text| {
            next.borrow_mut()
                .push(("second", text.to_owned(), view.get_cursor().col));
        })));
        owner.handle_input("\x1b[A");
    })));
    editor.handle_input("\x1b[A");
    assert_eq!(
        *events.borrow(),
        [("first", "new".into(), 0), ("second", "old".into(), 0)]
    );
    assert_eq!(editor.get_text(), "old");
    editor.set_on_change(None);
}
