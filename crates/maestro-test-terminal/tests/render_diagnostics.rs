//! Diagnostic files and errors the frame writer produces.

use std::path::PathBuf;
use std::rc::Rc;

use maestro_tui::{TUI, TerminalImage};

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod components;
    pub mod manual_runtime;
    pub mod recording_terminal;
}
use support::components::Probe;
use support::manual_runtime::{FileEffect, ManualRuntime};
use support::recording_terminal::RecordingTerminal;

/// Width of the terminals in these scenarios.
const COLUMNS: usize = 10;
/// Where the redraw log is appended.
const DEBUG_LOG: &str = "/home/fixture/.maestro/agent/maestro-debug.log";
/// Where the crash report is written.
const CRASH_LOG: &str = "/home/fixture/.maestro/agent/maestro-crash.log";

/// A writer with the given environment drawing `lines` on a 10 by 3 terminal.
struct Rig {
    /// The writer under test.
    tui: TUI,
    /// The recording terminal.
    terminal: RecordingTerminal,
    /// The controlled host.
    runtime: ManualRuntime,
    /// The only component.
    probe: Rc<Probe>,
}

impl Rig {
    /// Creates the writer; nothing is drawn yet.
    fn new(environment: &[(&str, &str)], lines: &[&str]) -> Self {
        let terminal = RecordingTerminal::new(COLUMNS, 3);
        let runtime = ManualRuntime::new();
        for (key, value) in environment {
            runtime.set_environment(key, value);
        }
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        let probe = Probe::shared(lines);
        tui.add_child(probe.clone());
        Self {
            tui,
            terminal,
            runtime,
            probe,
        }
    }

    /// Replaces the content and draws it.
    fn show(&self, lines: &[&str]) -> std::io::Result<()> {
        let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        self.probe.set_lines(&lines);
        self.tui.request_render(false);
        self.runtime.settle()
    }
}

/// Five lines, tall enough to scroll a three-row terminal.
const FIVE: [&str; 5] = ["one", "two", "three", "four", "five"];

/// One reason for a full redraw and what provokes it after `FIVE` was drawn.
struct Reason {
    /// Names the case in failure messages.
    name: &'static str,
    /// Changes the writer or its content.
    change: fn(&Rig),
    /// Full redraws once the reason has been logged.
    redraws: usize,
    /// The reason as logged.
    message: &'static str,
}

/// Replaces the content with `lines`.
fn show_lines(rig: &Rig, lines: &[&str]) {
    let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
    rig.probe.set_lines(&lines);
}

#[test]
fn redraw_log_reasons_keep_exact_messages() {
    let reasons = [
        Reason {
            name: "first",
            change: |_| (),
            redraws: 1,
            message: "first render (prev=0, new=5, height=3)",
        },
        Reason {
            name: "width",
            change: |rig| rig.terminal.resize(11, 3),
            redraws: 2,
            message: "terminal width changed (10 -> 11) (prev=5, new=5, height=3)",
        },
        Reason {
            name: "height",
            change: |rig| rig.terminal.resize(COLUMNS, 4),
            redraws: 2,
            message: "terminal height changed (3 -> 4) (prev=5, new=5, height=4)",
        },
        Reason {
            name: "shrink",
            change: |rig| {
                rig.tui.set_clear_on_shrink(true);
                show_lines(rig, &["one"]);
            },
            redraws: 2,
            message: "clearOnShrink (maxLinesRendered=5) (prev=5, new=1, height=3)",
        },
        Reason {
            name: "deleted",
            change: |rig| show_lines(rig, &["one"]),
            redraws: 2,
            message: "deleted lines moved viewport up (0 < 2) (prev=5, new=1, height=3)",
        },
        Reason {
            name: "above",
            change: |rig| show_lines(rig, &["ONE", "two", "three", "four", "five"]),
            redraws: 2,
            message: "firstChanged < viewportTop (0 < 2) (prev=5, new=5, height=3)",
        },
    ];
    for reason in reasons {
        let rig = Rig::new(&[("MAESTRO_DEBUG_REDRAW", "1")], &FIVE);
        if reason.name != "first" {
            rig.show(&FIVE).unwrap();
        }
        let mark = rig.runtime.files().len();
        (reason.change)(&rig);
        rig.tui.request_render(false);
        rig.runtime.settle().unwrap();
        let logged = FileEffect::Append(
            PathBuf::from(DEBUG_LOG),
            format!(
                "[2026-01-02T03:04:05.006Z] fullRender: {}\n",
                reason.message
            ),
        );
        assert_eq!(rig.runtime.files()[mark..], [logged], "{}", reason.name);
        assert_eq!(rig.tui.full_redraws(), reason.redraws, "{}", reason.name);
    }
}

#[test]
fn differential_debug_file_keeps_order_and_json() {
    let rig = Rig::new(&[("MAESTRO_TUI_DEBUG", "1")], &["old"]);
    rig.show(&["old"]).unwrap();
    let mark = rig.runtime.files().len();
    rig.show(&["new"]).unwrap();
    assert_eq!(
        rig.terminal.writes(),
        [
            "\x1b[?2026hold\x1b[0m\x1b]8;;\x07\x1b[?2026l",
            "\x1b[?25l",
            "\x1b[?2026h\r\x1b[2Knew\x1b[0m\x1b]8;;\x07\x1b[?2026l",
            "\x1b[?25l",
        ]
    );
    let expected = "firstChanged: 0\nviewportTop: 0\ncursorRow: 0\nheight: 3\nlineDiff: 0\n\
        hardwareCursorRow: 0\nrenderEnd: 0\nfinalCursorRow: 0\ncursorPos: null\n\
        newLines.length: 1\npreviousLines.length: 1\n\n=== newLines ===\n[\n  \
        \"new\\u001b[0m\\u001b]8;;\\u0007\"\n]\n\n=== previousLines ===\n[\n  \
        \"old\\u001b[0m\\u001b]8;;\\u0007\"\n]\n\n=== buffer ===\n\
        \"\\u001b[?2026h\\r\\u001b[2Knew\\u001b[0m\\u001b]8;;\\u0007\\u001b[?2026l\"";
    assert_eq!(
        rig.runtime.files()[mark..],
        [FileEffect::Write(
            PathBuf::from("/tmp/tui/render-1767323045006-i.log"),
            expected.to_owned()
        )]
    );
}

/// The error text for an oversized line.
fn overflow_message(row: usize, measured: usize) -> String {
    format!(
        "Rendered line {row} exceeds terminal width ({measured} > {COLUMNS}).\n\n\
        This is likely caused by a custom TUI component not truncating its output.\n\
        Use visibleWidth() to measure and truncateToWidth() to truncate lines.\n\n\
        Debug log written to: {CRASH_LOG}"
    )
}

/// The crash report for a frame of one oversized line.
fn crash_report() -> FileEffect {
    FileEffect::Write(
        PathBuf::from(CRASH_LOG),
        "Crash at 2026-01-02T03:04:05.006Z\nTerminal width: 10\nLine 0 visible width: 11\n\n\
        === All rendered lines ===\n[0] (w=11) 01234567890\x1b[0m\x1b]8;;\x07\n"
            .to_owned(),
    )
}

#[test]
fn overflow_reports_lines_then_restores_terminal() {
    let rig = Rig::new(&[], &["old"]);
    rig.show(&["old"]).unwrap();
    rig.terminal.clear_writes();
    let error = rig.show(&["01234567890"]).unwrap_err();
    assert_eq!(error.to_string(), overflow_message(0, 11));
    assert_eq!(rig.terminal.writes(), ["\x1b[1B", "\r\n", "\x1b[?25h"]);
    assert_eq!(rig.terminal.events().last(), Some(&"stop"));
    assert_eq!(rig.runtime.files(), [crash_report()]);
}

#[test]
fn first_frame_overflow_is_rejected() {
    let rig = Rig::new(&[], &["01234567890"]);
    rig.tui.start().unwrap();
    rig.terminal.clear_writes();
    let error = rig.runtime.settle().unwrap_err();
    assert_eq!(error.to_string(), overflow_message(0, 11));
    assert_eq!(
        rig.terminal.writes(),
        ["\x1b[?25h"],
        "no oversized line is drawn"
    );
    assert_eq!(rig.runtime.files(), [crash_report()]);

    let rig = Rig::new(&[], &["old"]);
    rig.show(&["old"]).unwrap();
    rig.terminal.clear_writes();
    rig.probe.set_lines(&["01234567890".to_owned()]);
    rig.tui.request_render(true);
    let error = rig.runtime.settle().unwrap_err();
    assert_eq!(error.to_string(), overflow_message(0, 11));
    assert_eq!(
        rig.terminal.writes(),
        ["\x1b[?25h"],
        "no oversized line is drawn"
    );
    assert_eq!(rig.runtime.files(), [crash_report()]);
}
