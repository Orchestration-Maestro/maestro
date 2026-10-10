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

#[test]
fn editor_narrow_rows_preserve_escape_payloads() {
    use maestro_tui::{Component, Editor, EditorOptions, tui::InputHandler};
    let _guard = support::globals();
    for escape in ["\x1b[31m", "\x1b]0;x\ty\x07"] {
        let (tui, _, _) = support::host(24);
        let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
        editor.handle_input(&format!("a{escape}bc"));
        assert_eq!(
            editor.render(2),
            [
                "──".to_owned(),
                "a ".to_owned(),
                format!("{escape}b "),
                "c\x1b[7m \x1b[0m".to_owned(),
                "──".to_owned()
            ]
        );
        assert_eq!(editor.get_text(), format!("a{escape}bc"));
    }
}

#[test]
fn editor_wraps_escape_interleaved_clusters_without_splitting_storage() {
    use maestro_tui::{Component, Editor, EditorOptions};
    let _guard = support::globals();
    let (tui, _, _) = support::host(24);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    for (line, rows) in [
        (
            "👩\x1b[31m\u{200d}💻x",
            vec!["👩\x1b[31m\u{200d}💻", "x\x1b[7m \x1b[0m"],
        ),
        (
            "\x1b[31m\u{1f3fd}x",
            vec!["\x1b[31m\u{1f3fd}", "x\x1b[7m \x1b[0m"],
        ),
        ("ab\x1b[0m", vec!["a ", "b\x1b[0m\x1b[7m \x1b[0m"]),
        (
            "a \x1b[31mbc",
            vec!["a ", "  ", "\x1b[31mb ", "c\x1b[7m \x1b[0m"],
        ),
    ] {
        editor.set_text(line);
        let mut expected = vec!["──".to_owned()];
        expected.extend(rows.into_iter().map(str::to_owned));
        expected.push("──".to_owned());
        assert_eq!(editor.render(2), expected, "{line:?}");
        assert_eq!(editor.get_text(), line);
        assert_eq!(editor.get_cursor().col, line.len());
    }
}

#[test]
fn editor_clipped_scroll_borders_keep_resets() {
    use maestro_tui::{Component, Editor, EditorOptions, tui::InputHandler};
    let _guard = support::globals();
    let (tui, _, _) = support::host(16);
    let editor = Editor::new(&tui, support::theme(), EditorOptions::default());
    editor.set_text(&["x"; 7].join("\n"));
    let reset = "\x1b[0m";
    let rows = editor.render(10);
    assert_eq!(rows[0], format!("─── ↑ 2{reset}...{reset}"));
    for _ in 0..6 {
        editor.handle_input("\x1b[A");
    }
    let rows = editor.render(10);
    assert_eq!(rows[0], "──────────");
    assert_eq!(rows[rows.len() - 1], format!("─── ↓ 2{reset}...{reset}"));
    editor.set_border_color(std::rc::Rc::new(|text| format!("\x1b[31m{text}")));
    let rows = editor.render(10);
    assert_eq!(
        rows[rows.len() - 1],
        format!("\x1b[31m─── ↓ 2{reset}...{reset}")
    );
}
