//! Input policy observes the selected terminal, not a byte re-scan.
use super::super::*;
use crate::{
    input::{KeyboardProtocol, MouseProtocolMode},
    panes::WheelRouting,
};
use maestro_terminal::Terminal;
use std::time::Instant;

#[test]
fn pane_bracketed_paste_restores_host_page_navigation() {
    let pane = PaneTerminal::new(Terminal::new(20, 3, 100).unwrap());
    assert_eq!(pane.plain_page_keys_use_host_scrollback(), Some(true));
    pane.seed_history_ansi("\x1b[?1h");
    assert_eq!(pane.plain_page_keys_use_host_scrollback(), Some(false));
    pane.seed_history_ansi("\x1b[?2004h");
    assert_eq!(pane.plain_page_keys_use_host_scrollback(), Some(true));
    assert!(
        pane.input_state()
            .unwrap()
            .plain_page_keys_use_host_scrollback()
    );
}

#[test]
fn live_modes_select_mouse_keyboard_and_wheel_policy() {
    let pane = PaneTerminal::new(Terminal::new(20, 3, 100).unwrap());
    assert_eq!(pane.wheel_routing(), Some(WheelRouting::HostScroll));
    assert_eq!(
        pane.keyboard_protocol(KeyboardProtocol::Legacy),
        KeyboardProtocol::Legacy
    );
    pane.seed_history_ansi("\x1b[?1049h\x1b[?1004h\x1b[?2004h\x1b[?1016h\x1b[>10u");
    assert!(pane.alternate_screen_active());
    assert!(pane.bracketed_paste_enabled());
    assert!(pane.focus_reporting_enabled());
    assert!(pane.sgr_pixel_mouse_enabled());
    assert!(pane.keyboard_report_all_requested());
    assert!(!pane.synchronized_output_active());
    assert_eq!(pane.wheel_routing(), Some(WheelRouting::AlternateScroll));
    pane.seed_history_ansi("\x1b[?1003h\x1b[?2031h\x1b[?2026h");
    assert_eq!(pane.wheel_routing(), Some(WheelRouting::MouseReport));
    assert!(pane.mouse_reporting_enabled());
    assert!(pane.synchronized_output_active());
    assert_eq!(
        pane.input_state().unwrap().mouse_protocol_mode,
        MouseProtocolMode::AnyMotion
    );
    assert!(!pane.plain_page_keys_use_host_scrollback().unwrap());
}

#[test]
fn seeded_history_is_retained_in_the_single_render_snapshot() {
    let pane = PaneTerminal::new(Terminal::new(20, 3, 100).unwrap());
    pane.seed_history_ansi("seeded");
    let text = pane
        .with_render_view(Instant::now(), |view| {
            view.render_state
                .rows()
                .flat_map(|row| {
                    row.cells()
                        .flat_map(|cell| cell.graphemes().collect::<Vec<_>>())
                        .collect::<Vec<_>>()
                })
                .collect::<String>()
        })
        .unwrap();
    assert!(text.contains("seeded"));
    assert_eq!(pane.visible_text(), "seeded\n");
    assert_eq!(
        pane.with_render_view(Instant::now(), |view| view.render_state.cols),
        Some(20)
    );
}

#[test]
fn input_handoff_modes_keep_flat_boolean_serialization() {
    let pane = PaneTerminal::new(Terminal::new(20, 3, 100).unwrap());
    pane.seed_history_ansi("\x1b[?2004h\x1b[?1004h\x1b[?1005h\x1b[?1000h\x1b[>4;2m");
    let state = pane.input_state().unwrap();
    let value = serde_json::to_value(state).unwrap();
    assert_eq!(value["bracketed_paste"], true);
    assert_eq!(value["focus_reporting"], true);
    assert_eq!(value["alternate_screen"], false);
    assert_eq!(value["modify_other_keys"], true);
    assert_eq!(serde_json::from_value::<InputState>(value).unwrap(), state);
}
