//! Diagnostic files and errors the frame writer produces.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use maestro_test_directory::Scratch;
#[cfg(unix)]
use maestro_tui::interface::TuiRuntime as _;
use maestro_tui::{TUI, TerminalImage};

use crate::scenarios::support::components::Probe;
use crate::scenarios::support::manual_runtime::ManualRuntime;
use crate::scenarios::support::recording_terminal::RecordingTerminal;

/// Width of the terminals in these scenarios.
const COLUMNS: usize = 10;
/// Directory that receives the records of every differential update.
#[cfg(unix)]
const RENDER_DIRECTORY: &str = "/tmp/tui";

/// Where the writer keeps `name` in the agent directory, joined natively.
fn agent_log(home: &Path, name: &str) -> PathBuf {
    agent_directory(home).join(name)
}

/// The agent directory the host supplies, which the writer creates only when it writes a
/// file.
fn agent_directory(home: &Path) -> PathBuf {
    home.join("agent")
}

/// The text the writer logged in `path`, or nothing when it wrote no file.
fn log_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Runs one diagnostics scenario in a child whose environment is exactly `environment`,
/// with a disposable home the writer logs below.
fn in_home(test: &str, environment: &[(&str, &str)], body: impl FnOnce(&Path)) {
    crate::scenarios::support::in_child(test, environment, || {
        let scratch = Scratch::new().unwrap();
        body(scratch.path());
    });
}

/// A writer with `home` drawing `lines` on a 10 by 3 terminal.
struct Rig {
    /// The writer under test.
    tui: TUI,
    /// The recording terminal.
    terminal: RecordingTerminal,
    /// The controlled host.
    runtime: ManualRuntime,
    /// The only component.
    probe: Rc<Probe>,
    /// The home directory the host reports.
    home: PathBuf,
}

impl Rig {
    /// Creates the writer; nothing is drawn yet and no log directory exists.
    fn new(home: &Path, lines: &[&str]) -> Self {
        let terminal = RecordingTerminal::new(COLUMNS, 3);
        let runtime = ManualRuntime::with_agent_dir(agent_directory(home));
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
            home: home.to_path_buf(),
        }
    }

    /// Replaces the content and draws it.
    fn show(&self, lines: &[&str]) -> std::io::Result<()> {
        let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        self.probe.set_lines(&lines);
        self.tui.request_render(false);
        self.runtime.settle()
    }

    /// The text the writer appended to the redraw log so far.
    fn redraw_log(&self) -> String {
        log_text(&agent_log(&self.home, "maestro-debug.log"))
    }

    /// The crash report the writer logged, if any.
    fn crash_log(&self) -> Option<String> {
        std::fs::read_to_string(agent_log(&self.home, "maestro-crash.log")).ok()
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

/// Every recorded reason for a full redraw and what provokes it.
fn redraw_reasons() -> [Reason; 6] {
    [
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
    ]
}

#[test]
fn redraw_log_reasons_keep_exact_messages() {
    in_home(
        "redraw_log_reasons_keep_exact_messages",
        &[("MAESTRO_DEBUG_REDRAW", "1")],
        |home| {
            for reason in redraw_reasons() {
                let rig = Rig::new(home, &FIVE);
                if reason.name != "first" {
                    rig.show(&FIVE).unwrap();
                }
                let mark = rig.redraw_log().len();
                (reason.change)(&rig);
                rig.tui.request_render(false);
                rig.runtime.settle().unwrap();
                let logged = format!(
                    "[2026-01-02T03:04:05.006Z] fullRender: {}\n",
                    reason.message
                );
                assert_eq!(rig.redraw_log()[mark..], logged, "{}", reason.name);
                assert_eq!(rig.tui.full_redraws(), reason.redraws, "{}", reason.name);
            }
        },
    );
}

#[test]
fn redraw_log_append_creates_its_directory() {
    in_home(
        "redraw_log_append_creates_its_directory",
        &[("MAESTRO_DEBUG_REDRAW", "1")],
        |home| {
            let rig = Rig::new(home, &FIVE);
            assert!(
                !agent_directory(home).exists(),
                "the agent directory is absent before the first frame"
            );
            rig.tui.start().unwrap();
            rig.tui.request_render(false);
            rig.runtime.settle().unwrap();
            assert_eq!(
                rig.redraw_log(),
                "[2026-01-02T03:04:05.006Z] fullRender: first render (prev=0, new=5, height=3)\n",
                "the append creates the directory and writes the line"
            );
        },
    );
}

#[test]
#[cfg(unix)]
fn differential_debug_file_keeps_order_and_json() {
    in_home(
        "differential_debug_file_keeps_order_and_json",
        &[("MAESTRO_TUI_DEBUG", "1")],
        |home| {
            let rig = Rig::new(home, &["old"]);
            rig.show(&["old"]).unwrap();
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
            let expected = "firstChanged: 0\nviewportTop: 0\ncursorRow: 0\nheight: 3\n\
                lineDiff: 0\nhardwareCursorRow: 0\nrenderEnd: 0\nfinalCursorRow: 0\n\
                cursorPos: null\nnewLines.length: 1\npreviousLines.length: 1\n\n\
                === newLines ===\n[\n  \"new\\u001b[0m\\u001b]8;;\\u0007\"\n]\n\n\
                === previousLines ===\n[\n  \"old\\u001b[0m\\u001b]8;;\\u0007\"\n]\n\n\
                === buffer ===\n\"\\u001b[?2026h\\r\\u001b[2Knew\\u001b[0m\\u001b]8;;\\u0007\\u001b[?2026l\"";
            let record = Path::new(RENDER_DIRECTORY).join(format!(
                "render-{}-{}.log",
                rig.runtime.log_context().unix_ms,
                rig.runtime.log_context().nonce
            ));
            let written = std::fs::read_to_string(&record).expect("the record is written");
            let _ = std::fs::remove_file(&record);
            assert_eq!(written, expected);
        },
    );
}

/// The error text for an oversized line, below `home`.
fn overflow_message(home: &Path, row: usize, measured: usize) -> String {
    format!(
        "Rendered line {row} exceeds terminal width ({measured} > {COLUMNS}).\n\n\
        This is likely caused by a custom TUI component not truncating its output.\n\
        Use visibleWidth() to measure and truncateToWidth() to truncate lines.\n\n\
        Debug log written to: {}",
        agent_log(home, "maestro-crash.log").display()
    )
}

/// The crash report for a frame of one oversized line.
const CRASH_REPORT: &str = "Crash at 2026-01-02T03:04:05.006Z\nTerminal width: 10\n\
    Line 0 visible width: 11\n\n=== All rendered lines ===\n\
    [0] (w=11) 01234567890\u{1b}[0m\u{1b}]8;;\u{7}\n";

#[test]
fn overflow_reports_lines_then_restores_terminal() {
    in_home(
        "overflow_reports_lines_then_restores_terminal",
        &[],
        |home| {
            let rig = Rig::new(home, &["old"]);
            assert!(
                !agent_directory(home).exists(),
                "the crash report finds no directory"
            );
            rig.show(&["old"]).unwrap();
            rig.terminal.clear_writes();
            let error = rig.show(&["01234567890"]).unwrap_err();
            assert_eq!(error.to_string(), overflow_message(home, 0, 11));
            assert_eq!(rig.terminal.writes(), ["\x1b[1B", "\r\n", "\x1b[?25h"]);
            assert_eq!(rig.terminal.events().last(), Some(&"stop"));
            assert_eq!(rig.crash_log().as_deref(), Some(CRASH_REPORT));
        },
    );
}

#[test]
fn first_frame_overflow_is_rejected() {
    in_home("first_frame_overflow_is_rejected", &[], |home| {
        let rig = Rig::new(home, &["01234567890"]);
        rig.tui.start().unwrap();
        rig.terminal.clear_writes();
        let error = rig.runtime.settle().unwrap_err();
        assert_eq!(error.to_string(), overflow_message(home, 0, 11));
        assert_eq!(
            rig.terminal.writes(),
            ["\x1b[?25h"],
            "no oversized line is drawn"
        );
        assert_eq!(rig.crash_log().as_deref(), Some(CRASH_REPORT));

        let rig = Rig::new(home, &["old"]);
        rig.show(&["old"]).unwrap();
        rig.terminal.clear_writes();
        rig.probe.set_lines(&["01234567890".to_owned()]);
        rig.tui.request_render(true);
        let error = rig.runtime.settle().unwrap_err();
        assert_eq!(error.to_string(), overflow_message(home, 0, 11));
        assert_eq!(
            rig.terminal.writes(),
            ["\x1b[?25h"],
            "no oversized line is drawn"
        );
        assert_eq!(rig.crash_log().as_deref(), Some(CRASH_REPORT));
    });
}

#[test]
fn settings_description_rows_fit_the_frame_viewport() {
    use maestro_tui::components::preferences::SettingsListOptions;
    use maestro_tui::{Component, SettingItem, SettingsList, SettingsListTheme};

    in_home(
        "settings_description_rows_fit_the_frame_viewport",
        &[],
        |home| {
            for width in [1, 2, 3, 4, 8] {
                let terminal = RecordingTerminal::new(width, 20);
                let runtime = ManualRuntime::with_agent_dir(agent_directory(home));
                let tui = TUI::new(
                    terminal.handle(),
                    runtime.handle(),
                    TerminalImage::new(|_| None, || 1),
                    None,
                );
                let list = Rc::new(SettingsList::new(
                    vec![SettingItem {
                        id: "a".into(),
                        label: "A".into(),
                        current_value: "v".into(),
                        description: Some("a界".into()),
                        values: None,
                        submenu: None,
                    }],
                    1,
                    SettingsListTheme {
                        label: Rc::new(|text, _| text.into()),
                        value: Rc::new(|text, _| text.into()),
                        description: Rc::new(|text| format!("\x1b[31m{text}!\x1b[0m")),
                        cursor: "→ ".into(),
                        hint: Rc::new(str::to_owned),
                    },
                    (Rc::new(|_, _| true), Rc::new(|| {})),
                    SettingsListOptions::default(),
                ));
                let lines = list.render(width);
                assert!(!lines[2].is_empty(), "description at width {width}");
                tui.add_child(list);
                tui.request_render(false);
                runtime.settle().unwrap_or_else(|error| {
                    panic!("description must fit the frame at width {width}: {error}")
                });
                assert!(
                    !agent_directory(home).exists(),
                    "no diagnostic file at width {width}"
                );
                assert!(!terminal.writes().is_empty());
            }
        },
    );
}
