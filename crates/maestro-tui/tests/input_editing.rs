//! Single-line input editing through its public component capability.
use maestro_tui::{Input, tui::InputHandler};
use std::{cell::RefCell, rc::Rc};
#[path = "fixtures/input_support/mod.rs"]
mod input_support;

#[test]
fn enter_submits_a_literal_backslash() {
    let _guard = input_support::globals();
    let input = Input::new();
    let submitted = Rc::new(RefCell::new(Vec::new()));
    let events = submitted.clone();
    input.set_on_submit(Some(Rc::new(move |value| {
        events.borrow_mut().push(value.to_owned());
    })));
    input.handle_input("hello\\");
    input.handle_input("\r");
    assert_eq!(*submitted.borrow(), ["hello\\"]);
}

#[test]
fn backslash_inserts_without_escape_mode() {
    let _guard = input_support::globals();
    let input = Input::new();
    input.handle_input("\\");
    input.handle_input("x");
    assert_eq!(input.get_value(), "\\x");
}
#[test]
fn backward_word_kill_yanks_at_start() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("foo bar baz".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "foo bar ");
    input0.handle_input("\u{1}");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "bazfoo bar ");
}
#[test]
fn line_start_kill_restores_its_prefix() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{15}");
    assert_eq!(input0.get_value(), "world");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "hello world");
}
#[test]
fn line_end_kill_restores_its_suffix() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{b}");
    assert_eq!(input0.get_value(), "");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn empty_kill_ring_leaves_input_alone() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("test".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "test");
}

#[test]
fn yank_pop_visits_each_entry_and_wraps() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("first".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("second".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("third".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "third");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "second");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "first");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "third");
}

#[test]
fn yank_pop_requires_an_active_yank() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("test".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("other".into());
    input0.handle_input("\u{5}");
    input0.handle_input("x");
    assert_eq!(input0.get_value(), "otherx");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "otherx");
}

#[test]
fn single_entry_yank_pop_leaves_text_alone() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("only".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "only");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "only");
}

#[test]
fn backward_kills_merge_in_reading_order() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("one two three".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.handle_input("\u{17}");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "one two three");
}

#[test]
fn typing_separates_kill_entries() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("foo bar baz".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "foo bar ");
    input0.handle_input("x");
    assert_eq!(input0.get_value(), "foo bar x");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "foo bar ");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "foo bar x");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "foo bar baz");
}

#[test]
fn typing_ends_the_yank_chain() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("first".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("second".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value(String::new());
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "second");
    input0.handle_input("x");
    assert_eq!(input0.get_value(), "secondx");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "secondx");
}

#[test]
fn a_rotated_kill_ring_keeps_its_new_top() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("first".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("second".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("third".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value(String::new());
    input0.handle_input("\u{19}");
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "second");
    input0.handle_input("x");
    input0.set_value(String::new());
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "second");
}

#[test]
fn forward_line_kill_retains_the_prefix() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("prefix|suffix".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{b}");
    assert_eq!(input0.get_value(), "prefix");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "prefix|suffix");
}

#[test]
fn forward_word_kills_append_in_order() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("hello world test".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}d");
    assert_eq!(input0.get_value(), " world test");
    input0.handle_input("\u{1b}d");
    assert_eq!(input0.get_value(), " test");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "hello world test");
}

#[test]
fn yank_inserts_between_existing_words() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("word".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "hello wordworld");
}

#[test]
fn yank_pop_replaces_only_the_inserted_middle_text() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("FIRST".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("SECOND".into());
    input0.handle_input("\u{5}");
    input0.handle_input("\u{17}");
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), concat!("hello SECOND", "world"));
    input0.handle_input("\u{1b}y");
    assert_eq!(input0.get_value(), "hello FIRSTworld");
}

#[test]
fn empty_undo_leaves_input_alone() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "");
}

#[test]
fn word_typing_undoes_in_runs() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input("w");
    input0.handle_input("o");
    input0.handle_input("r");
    input0.handle_input("l");
    input0.handle_input("d");
    assert_eq!(input0.get_value(), "hello world");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "");
}

#[test]
fn spaces_start_separate_undo_units() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input(" ");
    assert_eq!(input0.get_value(), "hello  ");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello ");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "");
}

#[test]
fn undo_restores_a_backward_deleted_grapheme() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input("\u{7f}");
    assert_eq!(input0.get_value(), "hell");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello");
}

#[test]
fn undo_restores_a_forward_deleted_grapheme() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[3~");
    assert_eq!(input0.get_value(), "hllo");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello");
}

#[test]
fn undo_restores_a_backward_killed_word() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input("w");
    input0.handle_input("o");
    input0.handle_input("r");
    input0.handle_input("l");
    input0.handle_input("d");
    assert_eq!(input0.get_value(), "hello world");
    input0.handle_input("\u{17}");
    assert_eq!(input0.get_value(), "hello ");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn undo_restores_a_killed_suffix() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input("w");
    input0.handle_input("o");
    input0.handle_input("r");
    input0.handle_input("l");
    input0.handle_input("d");
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{b}");
    assert_eq!(input0.get_value(), "hello ");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn undo_restores_a_killed_prefix() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input("w");
    input0.handle_input("o");
    input0.handle_input("r");
    input0.handle_input("l");
    input0.handle_input("d");
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{15}");
    assert_eq!(input0.get_value(), "world");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn undo_removes_a_yank() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("h");
    input0.handle_input("e");
    input0.handle_input("l");
    input0.handle_input("l");
    input0.handle_input("o");
    input0.handle_input(" ");
    input0.handle_input("\u{17}");
    input0.handle_input("\u{19}");
    assert_eq!(input0.get_value(), "hello ");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "");
}

#[test]
fn paste_undo_restores_the_preinsert_state() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[C");
    input0.handle_input("\u{1b}[200~beep boop\u{1b}[201~");
    assert_eq!(input0.get_value(), "hellobeep boop world");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn undo_restores_a_forward_killed_word() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.set_value("hello world".into());
    input0.handle_input("\u{1}");
    input0.handle_input("\u{1b}d");
    assert_eq!(input0.get_value(), " world");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "hello world");
}

#[test]
fn movement_separates_typing_undo_units() {
    let _guard = input_support::globals();
    let input0 = Input::new();
    input0.handle_input("a");
    input0.handle_input("b");
    input0.handle_input("c");
    input0.handle_input("\u{1}");
    input0.handle_input("\u{5}");
    input0.handle_input("d");
    input0.handle_input("e");
    assert_eq!(input0.get_value(), "abcde");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "abc");
    input0.handle_input("\u{1b}[45;5u");
    assert_eq!(input0.get_value(), "");
}

#[test]
fn value_replacement_retains_cursor_and_history() {
    input_support::run("value_replacement_retains_cursor_and_history");
}

#[test]
fn replacement_preserves_valid_logical_positions() {
    input_support::run("replacement_preserves_valid_logical_positions");
}

#[test]
fn replacement_ends_only_a_changed_yank_chain() {
    input_support::run("replacement_ends_only_a_changed_yank_chain");
}

#[test]
fn replacement_keeps_typing_kills_and_paste_state() {
    input_support::run("replacement_keeps_typing_kills_and_paste_state");
}

#[test]
fn callbacks_are_synchronous_replaceable_and_reentrant() {
    input_support::run("callbacks_are_synchronous_replaceable_and_reentrant");
}

#[test]
fn callbacks_and_ignored_keys_keep_typing_coalescence() {
    input_support::run("callbacks_and_ignored_keys_keep_typing_coalescence");
}

#[test]
fn configured_actions_replace_defaults() {
    input_support::run("configured_actions_replace_defaults");
}

#[test]
fn action_conflicts_follow_dispatch_order() {
    input_support::run("action_conflicts_follow_dispatch_order");
}

#[test]
fn binding_changes_apply_to_existing_inputs() {
    input_support::run("binding_changes_apply_to_existing_inputs");
}

#[test]
fn linefeed_submit_respects_earlier_cancel_and_undo() {
    input_support::run("linefeed_submit_respects_earlier_cancel_and_undo");
}

#[test]
fn printable_chunks_use_whitespace_anywhere_for_undo() {
    input_support::run("printable_chunks_use_whitespace_anywhere_for_undo");
}

#[test]
fn raw_key_aliases_reach_their_actions() {
    input_support::run("raw_key_aliases_reach_their_actions");
}

#[test]
fn raw_controls_are_rejected_as_whole_chunks() {
    input_support::run("raw_controls_are_rejected_as_whole_chunks");
}

#[test]
fn enhanced_printable_admission_uses_the_shared_decoder() {
    input_support::run("enhanced_printable_admission_uses_the_shared_decoder");
}

#[test]
fn inherited_key_fixes_do_not_insert_controls() {
    input_support::run("inherited_key_fixes_do_not_insert_controls");
}

#[test]
fn enhanced_actions_obey_current_protocol_state() {
    input_support::run("enhanced_actions_obey_current_protocol_state");
}

#[test]
fn grapheme_moves_and_deletes_use_local_segments() {
    input_support::run("grapheme_moves_and_deletes_use_local_segments");
}

#[test]
fn joining_text_keeps_prefix_suffix_segmentation() {
    input_support::run("joining_text_keeps_prefix_suffix_segmentation");
}

#[test]
fn word_navigation_uses_declared_whitespace() {
    input_support::run("word_navigation_uses_declared_whitespace");
}

#[test]
fn word_navigation_groups_punctuation_separately() {
    input_support::run("word_navigation_groups_punctuation_separately");
}

#[test]
fn boundary_actions_preserve_their_specific_coalescence() {
    input_support::run("boundary_actions_preserve_their_specific_coalescence");
}

#[test]
fn directional_kills_merge_without_losing_middle_text() {
    input_support::run("directional_kills_merge_without_losing_middle_text");
}

#[test]
fn yank_pop_is_undoable_without_undoing_ring_rotation() {
    input_support::run("yank_pop_is_undoable_without_undoing_ring_rotation");
}

#[test]
fn paste_framing_preserves_order_and_literal_payload() {
    input_support::run("paste_framing_preserves_order_and_literal_payload");
}
