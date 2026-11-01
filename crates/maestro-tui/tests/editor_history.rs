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
fn maestro_editor_does_nothing_on_up_arrow_when_history_is_empty() {
    support::run("maestro_editor_does_nothing_on_up_arrow_when_history_is_empty");
}

#[test]
fn maestro_editor_shows_most_recent_history_entry_on_up_arrow_when_editor_is_empty() {
    support::run("maestro_editor_shows_most_recent_history_entry_on_up_arrow_when_editor_is_empty");
}

#[test]
fn maestro_editor_cycles_through_history_entries_on_repeated_up_arrow() {
    support::run("maestro_editor_cycles_through_history_entries_on_repeated_up_arrow");
}

#[test]
fn maestro_editor_restores_the_draft_on_down_arrow_after_browsing_history() {
    support::run("maestro_editor_restores_the_draft_on_down_arrow_after_browsing_history");
}

#[test]
fn maestro_editor_navigates_forward_through_history_with_down_arrow() {
    support::run("maestro_editor_navigates_forward_through_history_with_down_arrow");
}

#[test]
fn maestro_editor_exits_history_mode_when_typing_a_character() {
    support::run("maestro_editor_exits_history_mode_when_typing_a_character");
}

#[test]
fn maestro_editor_exits_history_mode_on_settext() {
    support::run("maestro_editor_exits_history_mode_on_settext");
}

#[test]
fn maestro_editor_does_not_add_empty_strings_to_history() {
    support::run("maestro_editor_does_not_add_empty_strings_to_history");
}

#[test]
fn maestro_editor_does_not_add_consecutive_duplicates_to_history() {
    support::run("maestro_editor_does_not_add_consecutive_duplicates_to_history");
}

#[test]
fn maestro_editor_allows_non_consecutive_duplicates_in_history() {
    support::run("maestro_editor_allows_non_consecutive_duplicates_in_history");
}

#[test]
fn maestro_editor_uses_cursor_movement_instead_of_history_when_editor_has_content() {
    support::run("maestro_editor_uses_cursor_movement_instead_of_history_when_editor_has_content");
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
fn maestro_editor_allows_opposite_direction_cursor_movement_within_multi_line_history_entry() {
    support::run(
        "maestro_editor_allows_opposite_direction_cursor_movement_within_multi_line_history_entry",
    );
}

#[test]
fn maestro_editor_exits_history_browsing_mode_on_undo() {
    support::run("maestro_editor_exits_history_browsing_mode_on_undo");
}

#[test]
fn maestro_editor_undo_restores_to_pre_history_state_even_after_multiple_history_navigations() {
    support::run(
        "maestro_editor_undo_restores_to_pre_history_state_even_after_multiple_history_navigations",
    );
}

#[test]
fn history_callbacks_observe_committed_selection() {
    use maestro_tui::{Editor, EditorOptions, tui::InputHandler};
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
