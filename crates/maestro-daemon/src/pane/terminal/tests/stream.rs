//! PTY effects, host colours and ordered replies.

use super::super::*;
use maestro_terminal::{Terminal, color::Rgb};
use maestro_workspace::layout::PaneId;

fn current_palette_color(pane: &PaneTerminal, index: u8) -> Rgb {
    pane.core.lock().unwrap().terminal.colors.palette.current[usize::from(index)]
}

fn expected_osc_rgb_response(command: &str, color: Rgb) -> Vec<u8> {
    format!(
        "\x1b]{command};rgb:{:04x}/{:04x}/{:04x}\x1b\\",
        u16::from(color.r) * 257,
        u16::from(color.g) * 257,
        u16::from(color.b) * 257
    )
    .into_bytes()
}

#[test]
fn pane_process_pty_bytes_reports_latest_engine_pwd_callback() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let partial = pane.process_pty_bytes(pane_id, 0, b"\x1b]7;file:///tmp/maestro%20", || None);
    assert_eq!(partial.reported_cwd, None);

    let completed = pane.process_pty_bytes(pane_id, 0, b"repo\x07", || None);
    #[cfg(not(windows))]
    assert_eq!(
        completed.reported_cwd,
        Some(std::path::PathBuf::from("/tmp/maestro repo"))
    );
    #[cfg(windows)]
    assert_eq!(
        completed.reported_cwd,
        Some(std::path::PathBuf::from("\\tmp\\maestro repo"))
    );

    let latest = pane.process_pty_bytes(
        pane_id,
        0,
        b"\x1b]9;9;/tmp/conemu\x1b\\\x1b]1337;CurrentDir=/tmp/iterm2\x1b\\",
        || None,
    );
    assert_eq!(
        latest.reported_cwd,
        Some(std::path::PathBuf::from("/tmp/iterm2"))
    );
}

#[test]
fn pane_process_pty_bytes_surfaces_live_bells_only() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.seed_history_ansi("stale\x07");
    let result = pane.process_pty_bytes(pane_id, 0, b"\x07\x1b]0;title\x07\x07", || None);

    assert_eq!(result.terminal_bells, 2);
    let drained = pane.process_pty_bytes(pane_id, 0, b"live output", || None);
    assert_eq!(drained.terminal_bells, 0);
}

#[test]
fn pane_process_pty_bytes_reports_only_completed_title_changes() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    assert!(
        !pane
            .process_pty_bytes(pane_id, 0, b"\x1b]0;buil", || None)
            .terminal_title_changed
    );
    assert!(
        pane.process_pty_bytes(pane_id, 0, b"ding\x07", || None)
            .terminal_title_changed
    );
    assert!(
        !pane
            .process_pty_bytes(pane_id, 0, b"\x1b]2;building\x07", || None)
            .terminal_title_changed
    );
    assert!(
        pane.process_pty_bytes(pane_id, 0, b"\x1b]2;done\x07", || None)
            .terminal_title_changed
    );
}

#[test]
fn pane_process_pty_bytes_surfaces_clipboard_writes_without_other_results() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);

    let result = pane.process_pty_bytes(
        PaneId::from_raw(1),
        0,
        b"output\x1b]52;c;Y2xpcGJvYXJk\x07",
        || None,
    );

    assert!(result.request_render);
    assert_eq!(result.render_delay, None);
    assert_eq!(result.terminal_bells, 0);
    assert_eq!(result.clipboard_writes, vec![b"clipboard".to_vec()]);
    assert_eq!(result.reported_cwd, None);
    assert!(result.terminal_responses.is_empty());
}

#[test]
fn pane_seeded_history_clipboard_write_does_not_leak_into_live_output() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    pane.seed_history_ansi("\x1b]52;c;c3RhbGU=\x07");

    let result = pane.process_pty_bytes(PaneId::from_raw(1), 0, b"live output", || None);

    assert!(result.clipboard_writes.is_empty());
}

#[test]
fn pane_seeded_history_pwd_does_not_leak_into_live_output() {
    let terminal = Terminal::new(80, 24, 100).unwrap();
    let pane = PaneTerminal::new(terminal);
    pane.seed_history_ansi("\x1b]7;file:///tmp/restored\x07");

    let result = pane.process_pty_bytes(PaneId::from_raw(1), 0, b"live output", || None);

    assert_eq!(result.reported_cwd, None);
}

#[test]
fn pane_process_pty_bytes_does_not_advertise_unsupported_glyph_protocol() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b_25a1;s\x1b\\", || None);

    assert!(result.terminal_responses.is_empty());
}

#[test]
fn pane_process_pty_bytes_returns_engine_query_responses_without_queuing_input() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b[6n", || None);

    assert_eq!(result.terminal_responses.len(), 1);
    assert!(String::from_utf8_lossy(&result.terminal_responses[0]).contains('R'));
}

#[test]
fn pane_color_scheme_queries_and_live_updates_follow_terminal_mode() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    assert!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Dark))
            .is_none()
    );
    let query = pane.process_pty_bytes(pane_id, 0, b"\x1b[?996n", || None);
    assert_eq!(query.terminal_responses, vec![Vec::from(b"\x1b[?997;1n")]);

    pane.process_pty_bytes(pane_id, 0, b"\x1b[?2031h", || None);
    assert!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Dark))
            .is_none()
    );
    assert_eq!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Light)),
        Some(Vec::from(b"\x1b[?997;2n"))
    );

    assert!(pane.apply_host_terminal_appearance(None).is_none());
    let unknown_query = pane.process_pty_bytes(pane_id, 0, b"\x1b[?996n", || None);
    assert!(unknown_query.terminal_responses.is_empty());
    assert!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Dark))
            .is_none()
    );

    pane.process_pty_bytes(pane_id, 0, b"\x1bc", || None);
    assert!(
        pane.apply_host_terminal_appearance(Some(crate::terminal_theme::HostAppearance::Light))
            .is_none()
    );
}

#[test]
fn pane_process_pty_bytes_returns_xtgettcap_truecolor_query_responses_without_queuing_input() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(
        pane_id,
        0,
        b"\x1bP+q5463;524742;73657472676266;73657472676262\x1b\\",
        || None,
    );

    assert_eq!(
        result.terminal_responses,
        vec![
            Vec::from(b"\x1bP1+r5463\x1b\\".as_slice()),
            Vec::from(b"\x1bP1+r524742=38\x1b\\".as_slice()),
            Vec::from(b"\x1bP1+r73657472676266=5C455B33383A323A25703125643A25703225643A25703325646D\x1b\\".as_slice()),
            Vec::from(b"\x1bP1+r73657472676262=5C455B34383A323A25703125643A25703225643A25703325646D\x1b\\".as_slice()),
        ]
    );
}

#[test]
fn pane_process_pty_bytes_returns_split_xtgettcap_query_response() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1bP+q4", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"D73\x1b", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"\\", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(
            b"\x1bP1+r4D73=5C455D35323B25703125733B25703225735C303037\x1b\\".as_slice()
        )]
    );
}

#[test]
fn pane_process_pty_bytes_orders_device_attribute_reply_before_following_xtgettcap_reply() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b[c\x1bP+q5463\x1b\\", || None);

    assert_eq!(result.terminal_responses.len(), 2);
    assert!(String::from_utf8_lossy(&result.terminal_responses[0]).contains('c'));
    assert_eq!(
        result.terminal_responses[1],
        Vec::from(b"\x1bP1+r5463\x1b\\".as_slice())
    );
}

#[test]
fn pane_process_pty_bytes_orders_xtgettcap_reply_before_following_device_attribute_reply() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1bP+q5463\x1b\\\x1b[c", || None);

    assert_eq!(result.terminal_responses.len(), 2);
    assert_eq!(
        result.terminal_responses[0],
        Vec::from(b"\x1bP1+r5463\x1b\\".as_slice())
    );
    assert!(String::from_utf8_lossy(&result.terminal_responses[1]).contains('c'));
}

#[test]
fn pane_process_pty_bytes_orders_xtgettcap_reply_before_following_default_color_reply() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1bP+q5463\x1b\\\x1b]11;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![
            Vec::from(b"\x1bP1+r5463\x1b\\".as_slice()),
            Vec::from(b"\x1b]11;rgb:0000/2b2b/3636\x1b\\"),
        ]
    );
}

#[test]
fn pane_host_theme_update_preserves_child_default_color_override() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11;#112233\x07", || None);
    assert!(result.terminal_responses.is_empty());

    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11;?\x07", || None);
    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:1111/2222/3333\x07")]
    );
}

#[test]
fn pane_child_default_color_reset_restores_cached_host_color() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.process_pty_bytes(pane_id, 0, b"\x1b]11;#112233\x07", || None);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        }),
        ..Default::default()
    });
    pane.process_pty_bytes(pane_id, 0, b"\x1b]111\x07", || None);
    assert!(!pane.has_transient_default_color_override());

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11;?\x07", || None);
    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:aaaa/bbbb/cccc\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_recovers_xtgettcap_after_osc_bel_terminator() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]0;title\x07\x1bP+q5463\x1b\\", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1bP1+r5463\x1b\\".as_slice())]
    );
}

#[test]
fn pane_process_pty_bytes_ignores_unknown_and_unsupported_xtgettcap_queries() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1bP+q6E6F7065;4D7\x1b\\", || None);

    assert!(result.terminal_responses.is_empty());
}

#[test]
fn pane_process_pty_bytes_returns_underline_color_xtgettcap_query_responses() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(
        pane_id,
        0,
        b"\x1bP+q5375;536D756C78;536574756C63\x1b\\",
        || None,
    );

    assert_eq!(
        result.terminal_responses,
        vec![
            Vec::from(b"\x1bP1+r5375\x1b\\".as_slice()),
            Vec::from(b"\x1bP1+r536D756C78=5C455B343A25703125646D\x1b\\".as_slice()),
            Vec::from(b"\x1bP1+r536574756C63=5C455B35383A323A3A257031257B36353533367D252F25643A257031257B3235367D252F257B3235357D252625643A257031257B3235357D25262564253B6D\x1b\\".as_slice()),
        ]
    );
}

#[test]
fn pane_process_pty_bytes_orders_default_color_reply_before_following_device_attribute_reply() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11;?\x07\x1b[c", || None);

    assert_eq!(result.terminal_responses.len(), 2);
    assert_eq!(
        result.terminal_responses[0],
        Vec::from(b"\x1b]11;rgb:0000/2b2b/3636\x1b\\")
    );
    assert!(String::from_utf8_lossy(&result.terminal_responses[1]).contains('c'));
}

#[test]
fn pane_process_pty_bytes_returns_host_palette_color_without_queuing_input() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(
        crate::terminal_theme::TerminalTheme::default().with_palette_color(
            0,
            crate::terminal_theme::RgbColor {
                r: 0x11,
                g: 0x22,
                b: 0x33,
            },
        ),
    );

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;0;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]4;0;rgb:1111/2222/3333\x1b\\")]
    );
}

#[test]
fn pane_256_palette_query_burst_uses_host_snapshot() {
    use std::fmt::Write as _;

    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    let mut theme = crate::terminal_theme::TerminalTheme::default();
    let mut queries = String::new();
    for index in 0..=u8::MAX {
        theme = theme.with_palette_color(
            index,
            crate::terminal_theme::RgbColor {
                r: index,
                g: 0x22,
                b: 0x33,
            },
        );
        let _ = write!(queries, "\x1b]4;{index};?\x07");
    }
    pane.apply_host_terminal_theme(theme);

    let result = pane.process_pty_bytes(pane_id, 0, queries.as_bytes(), || None);

    assert_eq!(result.terminal_responses.len(), 256);
    assert_eq!(
        result.terminal_responses[0],
        Vec::from(b"\x1b]4;0;rgb:0000/2222/3333\x1b\\")
    );
    assert_eq!(
        result.terminal_responses[255],
        Vec::from(b"\x1b]4;255;rgb:ffff/2222/3333\x1b\\")
    );
}

#[test]
fn pane_child_palette_override_survives_host_refresh_until_reset() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(
        crate::terminal_theme::TerminalTheme::default().with_palette_color(
            7,
            crate::terminal_theme::RgbColor {
                r: 0x11,
                g: 0x22,
                b: 0x33,
            },
        ),
    );
    pane.process_pty_bytes(pane_id, 0, b"\x1b]4;7;rgb:aa/bb/cc\x1b\\", || None);

    pane.apply_host_terminal_theme(
        crate::terminal_theme::TerminalTheme::default().with_palette_color(
            7,
            crate::terminal_theme::RgbColor {
                r: 0x44,
                g: 0x55,
                b: 0x66,
            },
        ),
    );
    let overridden = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;7;?\x1b\\", || None);
    assert_eq!(
        overridden.terminal_responses,
        vec![Vec::from(b"\x1b]4;7;rgb:aaaa/bbbb/cccc\x1b\\")]
    );

    pane.process_pty_bytes(pane_id, 0, b"\x1b]104;7\x1b\\", || None);
    let reset = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;7;?\x1b\\", || None);
    assert_eq!(
        reset.terminal_responses,
        vec![Vec::from(b"\x1b]4;7;rgb:4444/5555/6666\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_returns_split_palette_color_query_response() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    let color = current_palette_color(&pane, 255);

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;25", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"5;?\x1b", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"\\", || None);

    assert_eq!(
        result.terminal_responses,
        vec![expected_osc_rgb_response("4;255", color)]
    );
}

#[test]
fn pane_process_pty_bytes_ignores_malformed_and_preserves_multi_palette_queries() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result = pane.process_pty_bytes(
        pane_id,
        0,
        b"\x1b]4;;?\x07\x1b]4;-1;?\x07\x1b]4;256;?\x07\x1b]4;0;?;1;?\x07\x1b]4;0;rgb:1111/2222/3333\x07",
        || None,
    );

    assert_eq!(result.terminal_responses.len(), 1);
    assert!(result.terminal_responses[0].starts_with(b"\x1b]4;0;rgb:"));
    assert_eq!(
        result.terminal_responses[0]
            .windows(4)
            .filter(|window| *window == b"rgb:")
            .count(),
        2
    );
}

#[test]
fn pane_process_pty_bytes_orders_palette_reply_before_following_terminal_replies() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    let color = current_palette_color(&pane, 0);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;0;?\x07\x1b]11;?\x07\x1b[c", || None);

    assert_eq!(result.terminal_responses.len(), 3);
    assert_eq!(
        result.terminal_responses[0],
        expected_osc_rgb_response("4;0", color)
    );
    assert_eq!(
        result.terminal_responses[1],
        Vec::from(b"\x1b]11;rgb:0000/2b2b/3636\x1b\\")
    );
    assert!(String::from_utf8_lossy(&result.terminal_responses[2]).contains('c'));
}

#[test]
fn pane_process_pty_bytes_returns_default_color_query_responses_without_queuing_input() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0x00,
            g: 0x2b,
            b: 0x36,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:0000/2b2b/3636\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_preserves_untracked_multi_color_query_responses() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }),
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }),
        ..Default::default()
    });

    let palette = pane.process_pty_bytes(pane_id, 0, b"\x1b]4;0;?;1;?\x1b\\", || None);
    let palette_response = palette.terminal_responses.concat();
    assert!(palette_response.starts_with(b"\x1b]4;0;rgb:"));
    assert_eq!(
        palette_response
            .windows(4)
            .filter(|window| *window == b"rgb:")
            .count(),
        2
    );

    let defaults = pane.process_pty_bytes(pane_id, 0, b"\x1b]10;?;?;?\x1b\\", || None);
    let default_response = defaults.terminal_responses.concat();
    assert!(
        default_response.starts_with(b"\x1b]10;rgb:"),
        "unexpected default-color report: {:?}",
        String::from_utf8_lossy(&default_response)
    );
    assert_eq!(
        default_response
            .windows(4)
            .filter(|window| *window == b"rgb:")
            .count(),
        3
    );
    let core = pane.core.lock().unwrap();
    assert!(!core.child_default_foreground_changed);
    assert!(!core.child_default_background_changed);
    drop(core);
}

#[test]
fn pane_process_pty_bytes_preserves_earlier_aggregate_palette_reply() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    let result =
        pane.process_pty_bytes(pane_id, 0, b"\x1b]4;0;?;1;?\x1b\\\x1b]4;0;?\x1b\\", || None);

    assert_eq!(result.terminal_responses.len(), 2);
    assert_eq!(
        result.terminal_responses[0]
            .windows(4)
            .filter(|window| *window == b"rgb:")
            .count(),
        2
    );
    assert!(result.terminal_responses[1].starts_with(b"\x1b]4;0;rgb:"));
}

#[test]
fn pane_process_pty_bytes_preserves_engine_reply_for_child_color_override() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.process_pty_bytes(pane_id, 0, b"\x1b]10;rgb:11/22/33\x07", || None);
    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]10;?\x1b\\", || None);

    assert_eq!(result.terminal_responses.len(), 1);
    assert!(result.terminal_responses[0].starts_with(b"\x1b]10;rgb:1111/2222/3333"));
}

#[test]
fn pane_process_pty_bytes_tracks_later_multi_value_color_set() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);

    pane.process_pty_bytes(pane_id, 0, b"\x1b]10;?;rgb:44/55/66\x1b\\", || None);

    let core = pane.core.lock().unwrap();
    assert!(!core.child_default_foreground_changed);
    assert!(core.child_default_background_changed);
}

#[test]
fn pane_process_pty_bytes_returns_cursor_color_query_response_from_foreground_fallback() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }),
        background: None,
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]12;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]12;rgb:6565/7b7b/8383\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_returns_cursor_color_query_response_from_child_foreground() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }),
        background: None,
        ..Default::default()
    });

    pane.process_pty_bytes(pane_id, 0, b"\x1b]10;rgb:11/22/33\x07", || None);
    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]12;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]12;rgb:1111/2222/3333\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_returns_explicit_cursor_color_query_response() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }),
        background: None,
        ..Default::default()
    });

    pane.process_pty_bytes(pane_id, 0, b"\x1b]12;rgb:11/22/33\x07", || None);
    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]12;?\x07", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]12;rgb:1111/2222/3333\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_returns_default_color_query_responses_in_order() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0x65,
            g: 0x7b,
            b: 0x83,
        }),
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(
        pane_id,
        0,
        b"\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07",
        || None,
    );

    assert_eq!(
        result.terminal_responses,
        vec![
            Vec::from(b"\x1b]10;rgb:6565/7b7b/8383\x1b\\"),
            Vec::from(b"\x1b]11;rgb:fdfd/f6f6/e3e3\x1b\\"),
            Vec::from(b"\x1b]12;rgb:6565/7b7b/8383\x1b\\"),
        ]
    );
}

#[test]
fn pane_process_pty_bytes_returns_split_default_color_query_response() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }),
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]11", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b";?\x1b", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"\\", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:fdfd/f6f6/e3e3\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_returns_split_cursor_color_query_response() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: Some(crate::terminal_theme::RgbColor {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }),
        background: None,
        ..Default::default()
    });

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]12", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b";?\x1b", || None);
    assert!(result.terminal_responses.is_empty());
    let result = pane.process_pty_bytes(pane_id, 0, b"\\", || None);

    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]12;rgb:fdfd/f6f6/e3e3\x1b\\")]
    );
}

#[test]
fn pane_process_pty_bytes_tracks_default_color_set_and_reset_before_replying() {
    let terminal = Terminal::new(20, 5, 0).unwrap();
    let pane = PaneTerminal::new(terminal);
    let pane_id = PaneId::from_raw(1);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme {
        foreground: None,
        background: Some(crate::terminal_theme::RgbColor {
            r: 0xfd,
            g: 0xf6,
            b: 0xe3,
        }),
        ..Default::default()
    });

    let result =
        pane.process_pty_bytes(pane_id, 0, b"\x1b]11;rgb:11/22/33\x07\x1b]11;?\x07", || {
            None
        });
    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:1111/2222/3333\x07")]
    );

    let result = pane.process_pty_bytes(pane_id, 0, b"\x1b]111\x07\x1b]11;?\x07", || None);
    assert_eq!(
        result.terminal_responses,
        vec![Vec::from(b"\x1b]11;rgb:fdfd/f6f6/e3e3\x1b\\")]
    );
}

#[test]
fn foreground_observations_are_only_requested_by_consuming_sequences() {
    for (bytes, shell_pid, alternate, expected) in [
        (b"ordinary output".as_slice(), 42, false, 0),
        (b"\x1b]11;?\x07", 42, false, 0),
        (b"\x1b[3J", 42, true, 0),
        (b"\x1b]11;#112233\x07", 0, false, 0),
        (b"\x1b]11;#112233\x07", 42, false, 1),
        (b"\x1b[3J", 42, false, 1),
        (
            b"\x1b]10;#112233\x07\x1b[3J\x1b]11;#445566\x07",
            42,
            false,
            1,
        ),
    ] {
        let pane = PaneTerminal::new(Terminal::new(20, 3, 100).unwrap());
        if alternate {
            pane.seed_history_ansi("\x1b[?1049h");
        }
        let calls = std::cell::Cell::new(0);
        pane.process_pty_bytes(PaneId::from_raw(1), shell_pid, bytes, || {
            calls.set(calls.get() + 1);
            None
        });
        assert_eq!(
            calls.get(),
            expected,
            "bytes={bytes:?}, shell={shell_pid}, alternate={alternate}"
        );
    }
}
