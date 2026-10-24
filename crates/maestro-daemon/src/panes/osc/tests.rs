//! Retained title, progress, directory and debug evidence cases.

use super::*;

fn enabled_osc_debug_tracker() -> OscDebugTracker {
    OscDebugTracker::new(true)
}

#[test]
fn pane_osc_stream_collector_ignores_strings_and_preserves_escaped_bytes() {
    let mut collector = OscStreamCollector::default();
    let mut bodies = Vec::new();

    collector.observe(
        b"\x1bPignored\x1b]0;not-osc\x07\x1b\\\x1b]9;a\x1b",
        |body, _| bodies.push(body.to_vec()),
    );
    collector.observe(b"\x1b\\\x1b]2;b\x1b\x07", |body, _| {
        bodies.push(body.to_vec());
    });

    assert_eq!(bodies, vec![b"9;a\x1b".to_vec(), b"2;b\x1b".to_vec()]);
}

#[test]
fn pane_reported_cwd_parses_file_uri_and_bare_paths() {
    assert_eq!(
        parse_reported_cwd(b"file:///tmp/maestro%20repo"),
        Some(std::path::PathBuf::from("/tmp/maestro repo"))
    );
    assert_eq!(
        parse_reported_cwd(b"C:\\Users\\maestro\\src\\maestro"),
        Some(std::path::PathBuf::from("C:\\Users\\maestro\\src\\maestro"))
    );
    assert_eq!(
        parse_reported_cwd(b"\"C:\\my proj\""),
        Some(std::path::PathBuf::from("C:\\my proj"))
    );
}

#[test]
fn pane_reported_cwd_rejects_invalid_or_empty_values() {
    assert_eq!(parse_reported_cwd(b""), None);
    assert_eq!(parse_reported_cwd(b"\xff"), None);
    assert_eq!(parse_reported_cwd(b"file://remote/tmp"), None);
}

#[test]
fn pane_agent_osc_osc0_title_with_bel() {
    let mut t = AgentOscStateTracker::default();
    t.observe("hello\x1b]0;braille title\x07world".as_bytes());
    assert_eq!(t.latest_title(), "braille title");
    assert_eq!(t.terminal_title(), Some("braille title"));
    assert_eq!(t.latest_progress(), "");
}

#[test]
fn pane_agent_osc_osc2_title_with_st() {
    let mut t = AgentOscStateTracker::default();
    t.observe("hello\x1b]2;static title\x1b\\world".as_bytes());
    assert_eq!(t.latest_title(), "static title");
    assert_eq!(t.latest_progress(), "");
}

#[test]
fn pane_agent_osc_empty_osc0_clears_title() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;some title\x07");
    assert_eq!(t.latest_title(), "some title");
    t.observe(b"\x1b]0;\x07");
    assert_eq!(t.latest_title(), "");
    assert_eq!(t.terminal_title(), None);
}

#[test]
fn pane_clearing_agent_evidence_preserves_the_terminal_title() {
    let mut tracker = AgentOscStateTracker::default();
    tracker.observe("\x1b]2;✳ 修复🙂标题\x1b\\".as_bytes());

    tracker.clear_retained();

    assert_eq!(tracker.latest_title(), "");
    assert_eq!(tracker.terminal_title(), Some("✳ 修复🙂标题"));
}

#[cfg(unix)]
#[test]
fn pane_handoff_seed_does_not_restore_agent_detection_evidence() {
    let mut tracker = AgentOscStateTracker::default();

    tracker.seed_terminal_title(Some("✳ restored title".into()));

    assert_eq!(tracker.terminal_title(), Some("✳ restored title"));
    assert_eq!(tracker.latest_title(), "");
}

#[test]
fn pane_agent_osc_osc9_sets_progress_with_bel() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]9;4;3;\x07");
    assert_eq!(t.latest_progress(), "4;3;");
    assert_eq!(t.latest_title(), "");
}

#[test]
fn pane_agent_osc_osc9_clear_progress_with_st() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]9;4;3;\x07");
    assert_eq!(t.latest_progress(), "4;3;");
    t.observe(b"\x1b]9;4;0;\x1b\\");
    assert_eq!(t.latest_progress(), "4;0;");
}

#[test]
fn pane_agent_osc_split_sequence_across_chunks() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]9;4;3");
    assert_eq!(t.latest_progress(), "");
    t.observe(b";\x07");
    assert_eq!(t.latest_progress(), "4;3;");
}

#[test]
fn pane_agent_osc_bel_and_st_terminators_both_work() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;title-bel\x07");
    assert_eq!(t.latest_title(), "title-bel");
    t.observe(b"\x1b]0;title-st\x1b\\");
    assert_eq!(t.latest_title(), "title-st");
}

#[test]
fn pane_agent_osc_oversized_payload_is_discarded_and_recovers() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;before\x07");
    assert_eq!(t.latest_title(), "before");

    let mut oversized = Vec::from(b"\x1b]0;".as_slice());
    oversized.extend(std::iter::repeat_n(b'x', 4097));
    oversized.push(0x07);
    t.observe(&oversized);
    assert_eq!(t.latest_title(), "before");

    t.observe(b"\x1b]0;after\x07");
    assert_eq!(t.latest_title(), "after");
}

#[test]
fn pane_agent_osc_cap_length_is_respected() {
    let mut t = AgentOscStateTracker::default();
    let long_title: String = "a".repeat(AGENT_OSC_MAX_CHARS + 50);
    let seq = format!("\x1b]0;{long_title}\x07");
    t.observe(seq.as_bytes());
    assert_eq!(t.latest_title().len(), AGENT_OSC_MAX_CHARS);
}

#[test]
fn pane_agent_osc_control_chars_stripped() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;before\x01after\x07");
    assert_eq!(t.latest_title(), "beforeafter");
}

#[test]
fn pane_agent_osc_unrelated_osc_does_not_overwrite_title() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;my title\x07");
    t.observe(b"\x1b]4;1;rgb:aa/bb/cc\x07");
    t.observe(b"\x1b]52;c;aGVsbG8=\x07");
    assert_eq!(t.latest_title(), "my title");
    assert_eq!(t.latest_progress(), "");
}

#[test]
fn pane_agent_osc_interleaved_sequences() {
    let mut t = AgentOscStateTracker::default();
    t.observe(b"\x1b]0;first\x07\x1b]9;4;3;\x07\x1b]2;second\x07");
    assert_eq!(t.latest_title(), "second");
    assert_eq!(t.latest_progress(), "4;3;");
}

#[test]
fn pane_agent_osc_default_state_is_empty() {
    let t = AgentOscStateTracker::default();
    assert_eq!(t.latest_title(), "");
    assert_eq!(t.latest_progress(), "");
}

#[test]
fn pane_osc_debug_tracker_detects_title_with_bel() {
    let mut tracker = enabled_osc_debug_tracker();

    tracker.observe("hello\x1b]0;✻ working title\x07world".as_bytes());

    assert_eq!(
        tracker.drain_pending(),
        vec![OscDebugEvent {
            command: "0".to_string(),
            payload: "✻ working title".to_string(),
        }]
    );
}

#[test]
fn pane_osc_debug_tracker_detects_title_with_st() {
    let mut tracker = enabled_osc_debug_tracker();

    tracker.observe("hello\x1b]2;static title\x1b\\world".as_bytes());

    assert_eq!(
        tracker.drain_pending(),
        vec![OscDebugEvent {
            command: "2".to_string(),
            payload: "static title".to_string(),
        }]
    );
}

#[test]
fn pane_osc_debug_tracker_detects_split_status_sequences() {
    let mut tracker = enabled_osc_debug_tracker();

    tracker.observe(b"\x1b]9;4;3");
    assert!(tracker.drain_pending().is_empty());
    tracker.observe(b"\x07\x1b]21337;status=working\x1b\\");

    assert_eq!(
        tracker.drain_pending(),
        vec![
            OscDebugEvent {
                command: "9".to_string(),
                payload: "4;3".to_string(),
            },
            OscDebugEvent {
                command: "21337".to_string(),
                payload: "status=working".to_string(),
            },
        ]
    );
}

#[test]
fn pane_osc_debug_tracker_ignores_untracked_osc_commands() {
    let mut tracker = enabled_osc_debug_tracker();

    tracker.observe(b"\x1b]52;c;SGVsbG8=\x07\x1b]7;file:///tmp\x07");

    assert!(tracker.drain_pending().is_empty());
}

#[test]
fn pane_osc_debug_tracker_sanitizes_control_characters() {
    let mut tracker = enabled_osc_debug_tracker();

    tracker.observe(b"\x1b]0;before\x01after\x07");

    assert_eq!(
        tracker.drain_pending(),
        vec![OscDebugEvent {
            command: "0".to_string(),
            payload: "beforeafter".to_string(),
        }]
    );
}

#[test]
fn pane_osc_debug_tracker_recovers_after_oversized_payload() {
    let mut tracker = enabled_osc_debug_tracker();
    let oversized = vec![b'a'; 4097];

    tracker.observe(b"\x1b]0;");
    tracker.observe(&oversized);
    tracker.observe(b"\x07\x1b]0;ok\x07");

    assert_eq!(
        tracker.drain_pending(),
        vec![OscDebugEvent {
            command: "0".to_string(),
            payload: "ok".to_string(),
        }]
    );
}

#[test]
fn pane_default_color_tracker_detects_split_osc_11_sequences() {
    let pane = super::super::display::PaneTerminal::new(
        maestro_terminal::Terminal::new(20, 5, 0).unwrap(),
    );
    let id = maestro_workspace::tiling::PaneId::from_raw(1);
    pane.process_pty_bytes(id, 0, b"\x1b]11;rgb:11/22", || None);
    pane.process_pty_bytes(id, 0, b"/33\x1b\\", || None);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme::default().with_color(
        crate::terminal_theme::DefaultColorKind::Background,
        crate::terminal_theme::RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        },
    ));
    assert_eq!(
        pane.process_pty_bytes(id, 0, b"\x1b]11;?\x1b\\", || None)
            .terminal_responses
            .concat(),
        b"\x1b]11;rgb:1111/2222/3333\x1b\\"
    );
}

#[test]
fn pane_default_color_tracker_ignores_osc_queries() {
    let pane = super::super::display::PaneTerminal::new(
        maestro_terminal::Terminal::new(20, 5, 0).unwrap(),
    );
    let id = maestro_workspace::tiling::PaneId::from_raw(1);
    pane.process_pty_bytes(id, 0, b"\x1b]10;?\x1b\\\x1b]11;?\x07", || None);
    pane.apply_host_terminal_theme(crate::terminal_theme::TerminalTheme::default().with_color(
        crate::terminal_theme::DefaultColorKind::Background,
        crate::terminal_theme::RgbColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
        },
    ));
    assert_eq!(
        pane.process_pty_bytes(id, 0, b"\x1b]11;?\x07", || None)
            .terminal_responses
            .concat(),
        b"\x1b]11;rgb:aaaa/bbbb/cccc\x1b\\"
    );
}
