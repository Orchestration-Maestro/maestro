#![cfg(test)]
//! Bracketed paste admission through the retained editor.
#[path = "fixtures/editor_support/mod.rs"]
mod support;
use maestro_tui::{Editor, EditorOptions, tui::InputHandler};
use std::{cell::RefCell, rc::Rc};
#[test]
fn paste_frames_buffer_until_end() {
    support::run("paste_frames_buffer_until_end");
}
#[test]
fn paste_jump_precedence_is_retained() {
    support::run("paste_jump_precedence_is_retained");
}
#[test]
fn paste_decodes_letters_before_normalization() {
    support::run("paste_decodes_letters_before_normalization");
}
#[test]
fn paste_path_spacing_is_ascii_and_precedes_threshold() {
    support::run("paste_path_spacing_is_ascii_and_precedes_threshold");
}
#[test]
fn paste_threshold_counts_normalized_scalars() {
    support::run("paste_threshold_counts_normalized_scalars");
}
#[test]
fn paste_splices_once_and_undoes_once() {
    support::run("paste_splices_once_and_undoes_once");
}
#[test]
fn paste_filtered_empty_still_captures_undo() {
    support::run("paste_filtered_empty_still_captures_undo");
}
#[test]
fn maestro_editor_undoes_single_line_paste_atomically() {
    support::run("maestro_editor_undoes_single_line_paste_atomically");
}
#[test]
fn maestro_editor_decodes_csi_u_ctrl_letter_sequences_inside_bracketed_paste_tmux_popup() {
    support::run(
        "maestro_editor_decodes_csi_u_ctrl_letter_sequences_inside_bracketed_paste_tmux_popup",
    );
}
#[test]
fn maestro_editor_undoes_multi_line_paste_atomically() {
    support::run("maestro_editor_undoes_multi_line_paste_atomically");
}
/// Callback observations as (text, extra) pairs.
type Events = Rc<RefCell<Vec<(String, String)>>>;
/// Change callbacks read the committed expanded payload.
fn change_reads_payload(editor: &Rc<Editor>, events: &Events, paste: &str, large: &str) {
    let reader = editor.clone();
    let log = events.clone();
    editor.set_on_change(Some(Rc::new(move |text| {
        log.borrow_mut()
            .push((text.to_owned(), reader.get_expanded_text()));
    })));
    editor.handle_input(paste);
    assert_eq!(
        events.take(),
        [("[paste #1 1001 chars]".to_owned(), large.to_owned())]
    );
}
/// A change callback may replace the text; undo restores the stored marker.
fn change_replaces_text(editor: &Rc<Editor>, events: &Events, paste: &str, large: &str) {
    editor.set_text("");
    events.take();
    let target = editor.clone();
    let log = events.clone();
    let once = std::cell::Cell::new(false);
    editor.set_on_change(Some(Rc::new(move |text| {
        log.borrow_mut().push((text.to_owned(), String::new()));
        if !once.replace(true) {
            target.set_text("replacement");
        }
    })));
    editor.handle_input(paste);
    assert_eq!(editor.get_text(), "replacement");
    let texts = events.take();
    assert_eq!(
        texts
            .iter()
            .map(|(text, _)| text.as_str())
            .collect::<Vec<_>>(),
        ["[paste #2 1001 chars]", "replacement"]
    );
    editor.handle_input("\x1b[45;5u");
    assert_eq!(editor.get_text(), "[paste #2 1001 chars]");
    assert_eq!(editor.get_expanded_text(), large);
    events.take();
}
/// Submission clears the stored pastes before the change callback and passes expanded text on.
fn submit_clears_before_change(editor: &Rc<Editor>, events: &Events, large: &str) {
    let observer = editor.clone();
    let log = events.clone();
    editor.set_on_change(Some(Rc::new(move |text| {
        if text.is_empty() {
            observer.set_text("[paste #1]");
            log.borrow_mut()
                .push(("cleared".to_owned(), observer.get_expanded_text()));
        }
    })));
    let log = events.clone();
    editor.set_on_submit(Some(Rc::new(move |text| {
        log.borrow_mut()
            .push(("submit".to_owned(), text.to_owned()));
    })));
    editor.handle_input("\r");
    assert_eq!(
        events.take(),
        [
            ("cleared".to_owned(), "[paste #1]".to_owned()),
            ("submit".to_owned(), large.to_owned())
        ]
    );
}
#[test]
fn paste_callbacks_read_committed_payload_and_allow_reentry() {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    let large = "x".repeat(1001);
    let paste = format!("\x1b[200~{large}\x1b[201~");
    let events = Events::default();
    change_reads_payload(&editor, &events, &paste, &large);
    change_replaces_text(&editor, &events, &paste, &large);
    submit_clears_before_change(&editor, &events, &large);
    editor.set_on_change(None);
    editor.set_on_submit(None);
    assert_eq!(Rc::strong_count(&editor), 1);
}
