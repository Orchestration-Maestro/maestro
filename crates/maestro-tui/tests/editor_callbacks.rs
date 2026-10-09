#![cfg(test)]
//! Callback reentry and ownership through the retained editor.
#[path = "fixtures/editor_support/host.rs"]
mod support;
use maestro_tui::{Editor, EditorOptions, tui::InputHandler};
use std::{cell::RefCell, rc::Rc};
#[test]
fn editor_change_callbacks_reenter_and_survive_replacement() {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    let events = Rc::new(RefCell::new(Vec::new()));
    let target = editor.clone();
    let log = events.clone();
    let nested = std::cell::Cell::new(false);
    editor.set_on_change(Some(Rc::new(move |text| {
        assert_eq!(target.get_cursor().line, 0);
        log.borrow_mut().push(format!(
            "old:{text}:{}:{}",
            target.get_text(),
            target.get_cursor().col
        ));
        if !nested.replace(true) {
            target.set_text("nested");
        }
    })));
    let retained = editor.on_change().unwrap();
    editor.handle_input("outer");
    let next = events.clone();
    editor.set_on_change(Some(Rc::new(move |text| {
        next.borrow_mut().push(format!("new:{text}"));
    })));
    editor.handle_input("!");
    editor.set_on_change(None);
    retained("held");
    assert_eq!(editor.get_text(), "nested!");
    assert_eq!(
        *events.borrow(),
        [
            "old:outer:outer:5",
            "old:nested:nested:6",
            "new:nested!",
            "old:held:nested!:7"
        ]
    );
    editor.set_on_change(None);
    drop(retained);
}
#[test]
fn editor_submit_reads_live_callback_after_change() {
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Rc::new(Editor::new(
        &tui,
        support::theme(),
        EditorOptions::default(),
    ));
    editor.set_text(" answer ");
    let events = Rc::new(RefCell::new(Vec::new()));
    let old = events.clone();
    editor.set_on_submit(Some(Rc::new(move |_| {
        old.borrow_mut().push("old-submit".to_owned());
    })));
    let target = editor.clone();
    let log = events.clone();
    editor.set_on_change(Some(Rc::new(move |value| {
        assert_eq!(value, "");
        assert_eq!(target.get_text(), "");
        assert_eq!(
            target.get_cursor(),
            maestro_tui::autocomplete::CursorPosition { line: 0, col: 0 }
        );
        log.borrow_mut().push("change-empty".to_owned());
        target.handle_input("\x1b[45;5u");
        assert_eq!(target.get_text(), "");
        log.borrow_mut().push("undo-after-clear".to_owned());
        target.set_on_change(None);
        let next = log.clone();
        let observed = Rc::downgrade(&target);
        target.set_on_submit(Some(Rc::new(move |text| {
            next.borrow_mut().push(format!(
                "new:{text}:{}",
                observed.upgrade().unwrap().get_text()
            ));
        })));
        target.insert_text_at_cursor("after");
    })));
    editor.handle_input("\r");
    assert_eq!(
        *events.borrow(),
        ["change-empty", "undo-after-clear", "new:answer:after"]
    );
    assert_eq!(editor.get_cursor().col, 5);
    editor.set_on_submit(None);
}
#[test]
fn editor_writer_and_callback_ownership_releases() {
    let runtime = support::manual_runtime::ManualRuntime::new();
    let runtime_handle = runtime.handle();
    let writer_witness = Rc::downgrade(&runtime_handle);
    let terminal = support::recording_terminal::RecordingTerminal::new(80, 24);
    let terminal_handle = terminal.handle();
    let terminal_witness = Rc::downgrade(&terminal_handle);
    let tui = maestro_tui::TUI::new(
        terminal_handle,
        runtime_handle,
        maestro_tui::TerminalImage::new(|_| None, || 1),
        None,
    );
    let theme_owner = Rc::new(());
    let theme_witness = Rc::downgrade(&theme_owner);
    let mut theme = support::theme();
    let selection: Rc<dyn Fn(&str) -> String> = Rc::new(move |text| {
        assert_eq!(Rc::strong_count(&theme_owner), 1);
        panic!("core rendering called selection styling: {text}");
    });
    theme.select_list = maestro_tui::SelectListTheme {
        selected_prefix: selection.clone(),
        selected_text: selection.clone(),
        description: selection.clone(),
        scroll_info: selection.clone(),
        no_match: selection,
    };
    let editor = Rc::new(Editor::new(&tui, theme, EditorOptions::default()));
    let editor_witness = Rc::downgrade(&editor);
    assert_eq!(maestro_tui::Component::render(&*editor, 8).len(), 3);
    let callback_owner = Rc::new(());
    let callback_witness = Rc::downgrade(&callback_owner);
    editor.set_on_change(Some(Rc::new(move |_| {
        assert_eq!(Rc::strong_count(&callback_owner), 1);
    })));
    let held = editor.on_change().unwrap();
    editor.set_on_change(None);
    assert!(callback_witness.upgrade().is_some());
    drop(held);
    assert!(callback_witness.upgrade().is_none());
    let submitted = Rc::new(std::cell::Cell::new(false));
    let value = submitted.clone();
    editor.set_on_submit(Some(Rc::new(move |text| {
        value.set(text == "alias");
    })));
    let held_submit = editor.on_submit().unwrap();
    editor.set_on_submit(None);
    held_submit("alias");
    assert!(submitted.get());
    drop(held_submit);
    tui.add_child(editor.clone());
    drop(editor);
    drop(tui);
    assert!(editor_witness.upgrade().is_none());
    assert!(writer_witness.upgrade().is_none());
    assert!(terminal_witness.upgrade().is_none());
    assert!(theme_witness.upgrade().is_none());
}
