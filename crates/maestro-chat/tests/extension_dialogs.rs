//! Extension dialogs replay complete controlled terminal observations.
#![cfg(test)]
#[path = "extension_dialog_support/mod.rs"]
mod extension_dialog_support;
use crate::manual_runtime;
use crate::published;
use crate::recording_terminal;
use crate::runtime_spy;

#[test]
fn selector_renders_bold_title_options_and_help() {
    extension_dialog_support::group("selector_renders_bold_title_options_and_help");
}

#[test]
fn input_renders_plain_title_editor_and_help() {
    extension_dialog_support::group("input_renders_plain_title_editor_and_help");
}

#[test]
fn selector_keeps_order_duplicates_and_empty_option_guard() {
    extension_dialog_support::group("selector_keeps_order_duplicates_and_empty_option_guard");
}

#[test]
fn selector_clamps_navigation_and_keeps_literal_shortcuts() {
    extension_dialog_support::group("selector_clamps_navigation_and_keeps_literal_shortcuts");
}

#[test]
fn dialogs_accept_lf_and_repeated_notifications() {
    extension_dialog_support::group("dialogs_accept_lf_and_repeated_notifications");
}

#[test]
fn dialogs_read_replaced_and_mutated_bindings() {
    extension_dialog_support::group("dialogs_read_replaced_and_mutated_bindings");
}

#[test]
fn dialog_key_collisions_preserve_source_precedence() {
    extension_dialog_support::group("dialog_key_collisions_preserve_source_precedence");
}

#[test]
fn dialog_hints_capture_constructor_bindings() {
    extension_dialog_support::group("dialog_hints_capture_constructor_bindings");
}

#[test]
fn input_accepts_but_does_not_render_placeholder() {
    extension_dialog_support::group("input_accepts_but_does_not_render_placeholder");
}

#[test]
fn input_delegates_editing_without_trimming_submission() {
    extension_dialog_support::group("input_delegates_editing_without_trimming_submission");
}

#[test]
fn input_forwards_focus_to_cursor_marker() {
    extension_dialog_support::group("input_forwards_focus_to_cursor_marker");
}

#[test]
fn input_focus_forwarding_keeps_parent_and_child_observations_separate() {
    extension_dialog_support::group(
        "input_focus_forwarding_keeps_parent_and_child_observations_separate",
    );
}

#[test]
fn dialog_colors_refresh_only_at_their_owning_operations() {
    extension_dialog_support::group("dialog_colors_refresh_only_at_their_owning_operations");
}

#[test]
fn dialog_countdown_requires_positive_timeout_and_writer() {
    extension_dialog_support::group("dialog_countdown_requires_positive_timeout_and_writer");
}

#[test]
fn dialog_countdown_reads_live_title_color_at_tick() {
    extension_dialog_support::group("dialog_countdown_reads_live_title_color_at_tick");
}

#[test]
fn dialog_dispose_stops_ticks_without_settling_callbacks() {
    extension_dialog_support::group("dialog_dispose_stops_ticks_without_settling_callbacks");
}

#[test]
fn dialog_notifications_do_not_implicitly_stop_countdown() {
    extension_dialog_support::group("dialog_notifications_do_not_implicitly_stop_countdown");
}

#[test]
fn dialog_children_remain_editable_and_retained_targets_survive_removal() {
    extension_dialog_support::group(
        "dialog_children_remain_editable_and_retained_targets_survive_removal",
    );
}

#[test]
fn dialog_callbacks_allow_reentry_and_disposal() {
    extension_dialog_support::group("dialog_callbacks_allow_reentry_and_disposal");
}

#[test]
fn dialog_constructor_propagates_hint_theme_failure() {
    extension_dialog_support::group("dialog_constructor_propagates_hint_theme_failure");
}

#[test]
fn dialogs_release_captures_and_cancel_work_with_containing_writer() {
    use extension_dialog_support::{Kind, colors, construct, writer};
    use maestro_chat::ExtensionSelectorOptions;
    use std::{cell::Cell, rc::Rc};
    extension_dialog_support::native(|| {
        for kind in [Kind::Selector, Kind::Input] {
            let spy = runtime_spy::Spy::default();
            let terminal = recording_terminal::RecordingTerminal::new(80, 24);
            let tui = writer(&spy, &terminal);
            let (_, theme) = published::published(&colors(0));
            let capture = Rc::new(Cell::new(0));
            let weak_capture = Rc::downgrade(&capture);
            let callback: Rc<dyn Fn()> = Rc::new(move || capture.set(capture.get() + 1));
            let weak_callback = Rc::downgrade(&callback);
            let dialog = Rc::new(
                construct(
                    kind,
                    (Rc::new(|_| {}), callback),
                    ExtensionSelectorOptions {
                        tui: Some(tui.clone()),
                        timeout: Some(1000.0),
                        runtime: Rc::new(spy.clone()),
                        theme,
                    },
                    ("Title".into(), vec!["Alpha".into()], None),
                )
                .unwrap(),
            );
            let weak_dialog = Rc::downgrade(&dialog);
            let child = dialog.container().children().borrow()[4].clone();
            let weak_child = Rc::downgrade(&child);
            tui.add_child(dialog.clone());
            tui.set_focus(Some(dialog.clone()));
            drop(child);
            drop(dialog);
            assert!(weak_dialog.upgrade().is_some());
            assert_eq!(spy.pending.get(), 1);
            drop(tui);
            assert!(weak_dialog.upgrade().is_none());
            assert!(weak_child.upgrade().is_none());
            assert!(weak_callback.upgrade().is_none());
            assert!(weak_capture.upgrade().is_none());
            assert_eq!(spy.pending.get(), 0);
            assert_eq!(spy.clock.pending(), 0);
        }
    });
}

#[test]
fn input_focus_routes_hardware_cursor_through_writer() {
    use extension_dialog_support::{Kind, advance, colors, native_dialog, writer};
    use std::rc::Rc;
    extension_dialog_support::native(|| {
        let spy = runtime_spy::Spy::default();
        let terminal = recording_terminal::RecordingTerminal::new(80, 24);
        let tui = writer(&spy, &terminal);
        tui.set_show_hardware_cursor(true).unwrap();
        let (_, theme) = published::published(&colors(0));
        let dialog = Rc::new(native_dialog(Kind::Input, theme, &spy, None));
        tui.add_child(dialog.clone());
        tui.set_focus(Some(dialog.clone()));
        tui.start().unwrap();
        advance(&spy, 16);
        let writes = terminal.writes().concat();
        assert!(!writes.contains(maestro_tui::CURSOR_MARKER));
        assert!(
            writes.ends_with("\x1b[4A\x1b[3G\x1b[?25h"),
            "cursor moves from the last border to the editor first cell"
        );
        assert!(writes.contains("\x1b[?25h"));
        terminal.clear_writes();
        terminal.send_input("界");
        advance(&spy, 32);
        assert!(terminal.writes().concat().contains("\x1b[5G"));
        tui.set_focus(None);
        assert!(!dialog.component().focusable().unwrap().focus_flag().get());
        tui.request_render(false);
        terminal.clear_writes();
        advance(&spy, 48);
        assert!(terminal.writes().concat().contains("\x1b[?25l"));
        tui.stop().unwrap();
        drop(tui);
        drop(dialog);
        assert_eq!(spy.clock.pending(), 0);
    });
}

use crate::palette;
#[test]
fn dialog_surface_uses_published_pack_roles() {
    use extension_dialog_support::{Kind, native_dialog};
    use maestro_tui::Component;
    extension_dialog_support::native(|| {
        for mode in ["dark", "light"] {
            let theme = palette::palette(mode);
            let (_, live) = published::published(&theme);
            let spy = runtime_spy::Spy::default();
            for kind in [Kind::Selector, Kind::Input] {
                let dialog = native_dialog(kind, live.clone(), &spy, None);
                assert_pack_rows(kind, &dialog.render(80), &theme);
            }
        }
    });
}

#[test]
fn dialogs_keep_plain_rows_within_one_column() {
    use extension_dialog_support::{Kind, colors, native_dialog};
    use maestro_tui::Component;
    extension_dialog_support::native(|| {
        let (_, theme) = published::published(&colors(0));
        let spy = runtime_spy::Spy::default();
        for kind in [Kind::Selector, Kind::Input] {
            let dialog = native_dialog(kind, theme.clone(), &spy, None);
            let rows = dialog.render(1);
            assert!(!rows.is_empty());
            assert!(rows.iter().all(|row| maestro_tui::visible_width(row) <= 1));
        }
    });
}

/// Assert each rendered branch against the supplied shipped theme's role consumers.
fn assert_pack_rows(
    kind: extension_dialog_support::Kind,
    rows: &[String],
    theme: &maestro_theme::Theme,
) {
    use extension_dialog_support::Kind;
    use maestro_theme::{Theme, ThemeColor};
    assert_eq!(
        rows[0],
        theme.fg(&ThemeColor::Border, &"─".repeat(80)).unwrap()
    );
    assert_eq!(rows.last().unwrap(), &rows[0]);
    let title = match kind {
        Kind::Selector => Theme::bold("Title", true),
        Kind::Input => "Title".into(),
    };
    assert_eq!(
        rows[2].trim_end(),
        " ".to_owned() + &theme.fg(&ThemeColor::Accent, &title).unwrap()
    );
    match kind {
        Kind::Selector => {
            assert_eq!(
                rows[4].trim_end(),
                " ".to_owned()
                    + &theme.fg(&ThemeColor::Accent, "→ ").unwrap()
                    + &theme.fg(&ThemeColor::Accent, "Alpha").unwrap()
            );
            assert_eq!(
                rows[5].trim_end(),
                "   ".to_owned() + &theme.fg(&ThemeColor::Text, "Beta").unwrap()
            );
        }
        Kind::Input => assert!(rows[4].starts_with("> \x1b[7m \x1b[27m")),
    }
    let hint = &rows[rows.len() - 3];
    assert!(hint.contains(&theme.fg(&ThemeColor::Dim, "enter").unwrap()));
    assert!(hint.contains(&theme.fg(&ThemeColor::Muted, " cancel").unwrap()));
}

#[test]
fn startup_selector_request_builds_existing_component() {
    use crate::settings_selection_support::plain;
    use maestro_app::presentation_data::input_selectors::ExtensionSelectorRequest;
    use maestro_tui::{Component, get_keybindings};
    use std::{cell::RefCell, rc::Rc};
    let _admission = crate::registry::REGISTRY.lock().unwrap();
    let _restore = crate::registry::Restore(get_keybindings());
    let extension_dialog_support::StartupHost {
        ui,
        terminal,
        factory,
        runtime,
    } = extension_dialog_support::startup_host();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let (selecting, cancelling) = (seen.clone(), seen.clone());
    let view = factory(ExtensionSelectorRequest {
        title: "Choose".into(),
        options: vec!["Continue".into(), "Cancel".into()],
        on_select: Rc::new(move |value| selecting.borrow_mut().push(value.to_owned())),
        on_cancel: Rc::new(move || cancelling.borrow_mut().push("cancelled".into())),
        tui: ui.clone(),
    })
    .unwrap();
    ui.add_child(view.component.clone());
    ui.set_focus(Some(view.focus));
    let first = ui.render(80).join("\n");
    assert!(first.contains("Continue") && first.contains("Cancel"));
    ui.start().unwrap();
    runtime.settle().unwrap();
    let painted = plain(&terminal.writes().concat());
    assert!(painted.contains("→ Continue") && painted.contains("  Cancel"));
    terminal.clear_writes();
    terminal.send_input("\u{e}");
    runtime.settle().unwrap();
    let moved = plain(&terminal.writes().concat());
    assert!(moved.contains("→ Cancel") && moved.contains("  Continue"));
    terminal.send_input("\u{19}");
    terminal.send_input("\u{18}");
    assert_eq!(seen.borrow().as_slice(), ["Cancel", "cancelled"]);
    for width in [3, 4, 8, 20] {
        assert!(
            ui.render(width)
                .iter()
                .all(|line| maestro_tui::visible_width(line) <= width)
        );
    }
    ui.stop().unwrap();
    assert_eq!(terminal.events(), ["start", "stop"]);
}
