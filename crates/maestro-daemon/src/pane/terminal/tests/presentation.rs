//! Render suppression, cursor observations and host-theme restoration.

use super::super::*;
use maestro_process::foreground::{ForegroundJob, ForegroundProcess};
use maestro_terminal::{CursorStyle, Terminal};
use maestro_workspace::layout::PaneId;
use std::time::{Duration, Instant};

fn pane() -> PaneTerminal {
    PaneTerminal::new(Terminal::new(20, 5, 100).unwrap())
}

fn process(pane: &PaneTerminal, bytes: &[u8]) -> ProcessBytesResult {
    pane.process_pty_bytes(PaneId::from_raw(1), 0, bytes, || None)
}

fn job(pid: u32, name: &str) -> ForegroundJob {
    ForegroundJob {
        process_group_id: pid,
        processes: vec![ForegroundProcess {
            pid,
            name: name.into(),
            argv0: Some(name.into()),
            argv: Some(vec![name.into()]),
            cmdline: Some(name.into()),
        }],
    }
}

fn host_theme() -> TerminalTheme {
    TerminalTheme::default().with_color(
        crate::terminal_theme::DefaultColorKind::Background,
        crate::terminal_theme::RgbColor {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        },
    )
}

#[test]
fn pane_synchronized_output_suppresses_intermediate_render_requests_until_batch_ends() {
    let pane = pane();
    let started = process(&pane, b"\x1b[?2026hfirst");
    assert!(!started.request_render);
    assert_eq!(started.render_delay, None);
    assert!(!process(&pane, b"next").request_render);
    assert!(process(&pane, b"\x1b[?2026l").request_render);
}

#[test]
fn pane_kitty_graphics_write_requests_render_with_settle_backstop() {
    let pane = pane();
    pane.set_kitty_graphics_enabled(true);
    let result = process(&pane, b"\x1b_Ga=d,d=A\x1b\\");
    assert!(result.request_render);
    assert_eq!(result.render_delay, Some(Duration::from_millis(20)));
    // Windows holds a moved cursor until it settles, so ordinary output waits for that.
    let settle = cfg!(windows).then_some(crate::pane::cursor::CURSOR_POSITION_SETTLE);
    assert_eq!(process(&pane, b"ordinary").render_delay, settle);
    let synchronized = process(&pane, b"\x1b[?2026h\x1b_Ga=d\x1b\\");
    assert!(!synchronized.request_render);
    assert_eq!(synchronized.render_delay, None);
    process(&pane, b"\x1b[?2026l");
    pane.set_kitty_graphics_enabled(false);
    assert_eq!(process(&pane, b"\x1b_Ga=d\x1b\\").render_delay, settle);
}

#[test]
fn pane_decscusr_cursor_shape_preserves_blinking_variants() {
    for (style, blinking, shape) in [
        (CursorStyle::Block, true, 1),
        (CursorStyle::BlockHollow, true, 1),
        (CursorStyle::Block, false, 2),
        (CursorStyle::BlockHollow, false, 2),
        (CursorStyle::Underline, true, 3),
        (CursorStyle::Underline, false, 4),
        (CursorStyle::Bar, true, 5),
        (CursorStyle::Bar, false, 6),
    ] {
        assert_eq!(
            super::super::cursor::decscusr_cursor_shape(style, blinking),
            shape
        );
    }
}

#[test]
fn pane_cursor_state_uses_terminal_default_until_child_sets_shape() {
    let pane = pane();
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 0);
    process(&pane, b"\x1b[3 q");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 3);
}

#[test]
fn pane_cursor_state_returns_terminal_default_after_decscusr_reset() {
    let pane = pane();
    process(&pane, b"\x1b[6 q");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 6);
    process(&pane, b"\x1b[0 q");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 0);
    process(&pane, b"\x1b[4 q\x1bc");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 0);
}

#[test]
fn pane_cursor_shape_tracker_handles_split_decscusr_sequences() {
    let pane = pane();
    process(&pane, b"\x1b[");
    process(&pane, b"5 ");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 0);
    process(&pane, b"q");
    assert_eq!(pane.cursor_state(Instant::now()).unwrap().shape, 5);
}

#[test]
fn pane_host_terminal_theme_restore_probe_skips_when_no_transient_override() {
    let pane = pane();
    pane.apply_host_terminal_theme(host_theme());
    assert!(!pane.should_probe_host_terminal_theme_restore());
}

#[test]
fn pane_host_terminal_theme_restore_probe_skips_when_host_theme_unknown() {
    let pane = pane();
    pane.process_pty_bytes(PaneId::from_raw(1), 7, b"\x1b]11;#aabbcc\x07", || {
        Some(job(42, "child"))
    });
    assert!(pane.has_transient_default_color_override());
    assert!(!pane.should_probe_host_terminal_theme_restore());
}

#[test]
fn pane_host_terminal_theme_restore_probe_skips_on_alternate_screen() {
    let pane = pane();
    pane.apply_host_terminal_theme(host_theme());
    pane.process_pty_bytes(
        PaneId::from_raw(1),
        7,
        b"\x1b]11;#aabbcc\x07\x1b[?1049h",
        || Some(job(42, "child")),
    );
    assert!(!pane.should_probe_host_terminal_theme_restore());
}

#[test]
fn pane_host_terminal_theme_restore_probe_runs_when_restore_is_pending() {
    let pane = pane();
    pane.apply_host_terminal_theme(host_theme());
    pane.process_pty_bytes(PaneId::from_raw(1), 7, b"\x1b]11;#aabbcc\x07", || {
        Some(job(42, "child"))
    });
    assert!(pane.should_probe_host_terminal_theme_restore());
}

#[test]
fn pane_restore_host_terminal_theme_reapplies_cached_colors() {
    let pane = pane();
    pane.apply_host_terminal_theme(host_theme());
    pane.process_pty_bytes(PaneId::from_raw(1), 7, b"\x1b]11;#aabbcc\x07", || {
        Some(job(42, "child"))
    });
    assert!(!pane.maybe_restore_host_terminal_theme(7, Some(&job(42, "child"))));
    assert!(!pane.maybe_restore_host_terminal_theme(7, None));
    assert!(pane.maybe_restore_host_terminal_theme(7, Some(&job(7, "zsh"))));
    assert!(!pane.has_transient_default_color_override());
    assert_eq!(
        process(&pane, b"\x1b]11;?\x07").terminal_responses.concat(),
        b"\x1b]11;rgb:1111/2222/3333\x1b\\"
    );
}

#[test]
fn pane_seed_title_and_clear_agent_evidence_preserve_display_title() {
    let pane = pane();
    pane.seed_terminal_title(Some("restored".into()));
    assert_eq!(pane.terminal_title().as_deref(), Some("restored"));
    assert_eq!(pane.agent_osc_title(), "");
    process(&pane, b"\x1b]2;active\x07\x1b]9;4;3;\x07");
    assert_eq!(pane.agent_osc_title(), "active");
    assert_eq!(pane.agent_osc_progress(), "4;3;");
    pane.clear_agent_osc_state();
    assert_eq!(pane.terminal_title().as_deref(), Some("active"));
    assert_eq!(pane.agent_osc_title(), "");
    assert_eq!(pane.agent_osc_progress(), "");
}

#[test]
fn pane_host_appearance_does_not_consume_pending_history_replies() {
    let pane = pane();
    pane.seed_history_ansi("\x1b[5n");
    assert!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Dark))
            .is_none()
    );
    assert_eq!(
        process(&pane, b"\x1bP+q5463\x1b\\")
            .terminal_responses
            .concat(),
        b"\x1b[0n\x1bP1+r5463\x1b\\"
    );
}

#[test]
#[cfg(windows)]
fn pane_cursor_state_holds_pty_position_change_until_settle_window() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.process_pty_bytes(pane_id, 0, b"x", || None);
    assert_eq!(
        pane.cursor_state(Instant::now())
            .map(|cursor| (cursor.x, cursor.y, cursor.visible)),
        Some((1, 0, true))
    );

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b[6;21H", || None);

    assert_eq!(
        result.render_delay,
        Some(crate::pane::cursor::CURSOR_POSITION_SETTLE)
    );
    assert_eq!(
        pane.cursor_state(Instant::now())
            .map(|cursor| (cursor.x, cursor.y, cursor.visible)),
        Some((1, 0, true))
    );
}

#[test]
#[cfg(not(windows))]
fn pane_cursor_state_uses_live_position_when_settle_policy_disabled() {
    let terminal = Terminal::new(80, 24, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.process_pty_bytes(pane_id, 0, b"x", || None);
    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b[6;21H", || None);

    assert_eq!(result.render_delay, None);
    assert_eq!(
        pane.cursor_state(Instant::now())
            .map(|cursor| (cursor.x, cursor.y, cursor.visible)),
        Some((20, 5, true))
    );
}

#[test]
fn pane_cursor_settle_policy_controls_render_delay() {
    let pane = pane();
    let mut core = pane.core.lock().unwrap();
    let now = Instant::now();
    let initial = core.current_cursor_state();
    core.cursor_settle_state.observe(initial, now);
    core.terminal.write(b"x");
    let moved = core.current_cursor_state();
    core.cursor_settle_state.observe(moved, now);
    assert!(core.cursor_settle_state.pending());
    assert_eq!(
        core.render_delay_after_pty_write(false),
        if cfg!(windows) {
            Some(crate::pane::cursor::CURSOR_POSITION_SETTLE)
        } else {
            None
        }
    );
    assert_eq!(
        core.render_delay_after_pty_write(true),
        Some(Duration::from_millis(20))
    );
    core.terminal.write(b"\x1b[?2026h");
    assert_eq!(core.render_delay_after_pty_write(true), None);
}
