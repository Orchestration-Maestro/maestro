#![cfg(test)]
//! Exact multiline editor rows through the component boundary.
#[path = "fixtures/editor_support/mod.rs"]
mod support;
#[test]
fn editor_render_wraps_and_pads() {
    support::run("editor_render_wraps_and_pads");
}

#[test]
fn editor_end_cursor_borrows_padding() {
    support::run("editor_end_cursor_borrows_padding");
}

#[test]
fn editor_focus_emits_marker_without_changing_text() {
    support::run("editor_focus_emits_marker_without_changing_text");
}

#[test]
fn editor_scroll_tracks_cursor_and_live_terminal_rows() {
    support::run("editor_scroll_tracks_cursor_and_live_terminal_rows");
}

#[test]
fn editor_narrow_viewport_keeps_cursor_and_border_bounded() {
    support::run("editor_narrow_viewport_keeps_cursor_and_border_bounded");
}

#[test]
fn editor_bottom_scroll_indicator_is_clipped() {
    support::run("editor_bottom_scroll_indicator_is_clipped");
}

#[test]
fn editor_display_tabs_do_not_change_stored_cursor() {
    support::run("editor_display_tabs_do_not_change_stored_cursor");
}

#[test]
fn editor_escape_payload_tabs_are_not_display_spaces() {
    support::run("editor_escape_payload_tabs_are_not_display_spaces");
}

#[test]
fn editor_cursor_inside_escape_moves_only_for_display() {
    use maestro_tui::{Component, Editor, EditorOptions, tui::InputHandler};
    support::run("editor_cursor_inside_escape_moves_only_for_display");
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    editor.handle_input("a\x1b]0;x\ty\x07b");
    editor.handle_input("\x1b[D");
    editor.handle_input("\x1b[D");
    assert_eq!(
        editor.render(20),
        [
            "─".repeat(20),
            format!("a\x1b]0;x\ty\x07\x1b[7mb\x1b[0m{}", " ".repeat(18)),
            "─".repeat(20)
        ]
    );
    assert_eq!(editor.get_cursor().col, 8);
    editor.handle_input("Z");
    assert_eq!(editor.get_text(), "a\x1b]0;x\tyZ\x07b");
}
#[test]
fn editor_border_callback_is_live_before_layout() {
    use maestro_tui::{Component, Editor, EditorOptions};
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
    editor.set_text("old");
    let target = editor.clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let log = calls.clone();
    let changed = Cell::new(false);
    editor.set_border_color(Rc::new(move |text| {
        log.borrow_mut().push(text.to_owned());
        if !changed.replace(true) {
            target.set_text("new");
        }
        format!("<{text}>")
    }));
    let held = editor.border_color();
    assert_eq!(
        editor.render(8),
        [
            "<─>".repeat(8),
            "new\x1b[7m \x1b[0m    ".to_owned(),
            "<─>".repeat(8)
        ]
    );
    editor.set_border_color(Rc::new(|text| format!("[{text}]")));
    assert_eq!(
        editor.render(8),
        [
            "[─]".repeat(8),
            "new\x1b[7m \x1b[0m    ".to_owned(),
            "[─]".repeat(8)
        ]
    );
    assert_eq!(held("x"), "<x>");
    assert_eq!(*calls.borrow(), ["─", "x"]);
    assert_eq!(editor.get_text(), "new");
    drop(held);
}
