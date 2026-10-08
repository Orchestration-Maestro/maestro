//! Process terminal output: exact bytes, progress, dimensions and the write log. Each
//! scenario runs in a child process that owns its standard descriptors and environment.
#![cfg(test)]
#![cfg(unix)]

#[macro_use]
mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::stdio::stdout;
use support::{
    Capture, Inputs, Resizes, Rig, START_BYTES, advance, is_child, pipe_input, pipe_output,
    pty_output, settle,
};

/// Collects on another thread until `length` bytes arrived, so a writer can fill the pipe and
/// wait for room.
fn collect(capture: Capture, length: usize) -> JoinHandle<String> {
    std::thread::spawn(move || {
        let mut collected = String::new();
        while collected.len() < length {
            let mut waiting = [PollFd::new(&capture.0, PollFlags::IN)];
            poll(&mut waiting, None).unwrap();
            collected.push_str(&capture.take());
        }
        collected
    })
}

/// A window size as the terminal reports it: columns and rows.
type Native = (u16, u16);

/// The progress sequences.
const PROGRESS_ACTIVE: &str = "\x1b]9;4;3\x07";

/// The sequence that clears progress.
const PROGRESS_CLEAR: &str = "\x1b]9;4;0;\x07";

#[test]
fn native_writes_exact_cursor_clear_title_bytes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        let mut terminal = ProcessTerminal::new(rig.local());
        terminal.write("a\n\u{feff}\u{85}").unwrap();
        assert_eq!(output.take(), "a\n\u{feff}\u{85}");
        let moves: [(isize, &str); 5] = [
            (-3, "\x1b[3A"),
            (0, ""),
            (4, "\x1b[4B"),
            (isize::MIN, "\x1b[9223372036854775808A"),
            (isize::MAX, "\x1b[9223372036854775807B"),
        ];
        for (lines, expected) in moves {
            terminal.move_by(lines).unwrap();
            assert_eq!(output.take(), expected, "{lines}");
        }
        terminal.hide_cursor().unwrap();
        terminal.show_cursor().unwrap();
        terminal.clear_line().unwrap();
        terminal.clear_from_cursor().unwrap();
        terminal.clear_screen().unwrap();
        terminal.set_title("x\x07\x1b]0;y").unwrap();
        let expected = "\x1b[?25l\x1b[?25h\x1b[K\x1b[J\x1b[2J\x1b[H\x1b]0;x\x07\x1b]0;y\x07";
        assert_eq!(output.take(), expected);
    });
}

#[test]
fn native_writes_complete_on_a_nonblocking_standard_output() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let capture = pipe_output();
        let flags = fcntl_getfl(stdout()).unwrap();
        fcntl_setfl(stdout(), flags | OFlags::NONBLOCK).unwrap();
        let mut terminal = ProcessTerminal::new(rig.local());
        let data = "x".repeat(300_000);
        let collected = collect(capture, data.len());
        terminal.write(&data).unwrap();
        let received = collected.join().unwrap();
        assert_eq!(received.len(), data.len());
        assert!(received.bytes().all(|byte| byte == b'x'));
    });
}

#[test]
fn native_progress_repeats_one_keepalive_until_clear() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal.set_progress(true).unwrap();
            assert_eq!(output.take(), PROGRESS_ACTIVE);
            advance(&rig.local(), 999).await;
            assert_eq!(output.take(), "");
            terminal.set_progress(true).unwrap();
            assert_eq!(output.take(), PROGRESS_ACTIVE);
            advance(&rig.local(), 1).await;
            assert_eq!(output.take(), PROGRESS_ACTIVE);
            advance(&rig.local(), 1000).await;
            assert_eq!(output.take(), PROGRESS_ACTIVE);
            terminal.set_progress(false).unwrap();
            assert_eq!(output.take(), PROGRESS_CLEAR);
            advance(&rig.local(), 2000).await;
            assert_eq!(output.take(), "");
        });
    });
}

#[test]
fn native_stop_clears_active_progress_before_input_modes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let feed = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let (inputs, resizes) = (Inputs::default(), Resizes::default());
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal
                .start(inputs.callback(), resizes.callback())
                .unwrap();
            feed.send(b"\x1b[?1u");
            settle(&rig.local()).await;
            terminal.set_progress(true).unwrap();
            assert_eq!(
                output.take(),
                format!("{START_BYTES}\x1b[>7u{PROGRESS_ACTIVE}")
            );
            terminal.stop().unwrap();
            assert_eq!(output.take(), format!("{PROGRESS_CLEAR}\x1b[?2004l\x1b[<u"));
            advance(&rig.local(), 3000).await;
            assert_eq!(output.take(), "");
        });
    });
}

#[test]
fn native_clear_progress_emits_even_when_inactive() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pipe_output();
        let mut terminal = ProcessTerminal::new(rig.local());
        terminal.set_progress(false).unwrap();
        assert_eq!(output.take(), PROGRESS_CLEAR);
        terminal.stop().unwrap();
        assert_eq!(output.take(), "\x1b[?2004l");
    });
}

/// Environment values and the dimensions they select when standard output reports none.
fn environment_cases() -> Vec<(String, (usize, usize))> {
    let defaults = (80, 24);
    let both = |value: usize| (value, value);
    let accepted = [
        ("123", both(123)),
        (" 123 ", both(123)),
        ("\u{c}123\u{c}", both(123)),
        ("\t123\r\n", both(123)),
        ("00080", both(80)),
        ("9007199254740993", both(9_007_199_254_740_993)),
    ];
    let rejected = [
        "",
        " ",
        "0",
        "-0",
        "\u{feff}123\u{feff}",
        "\u{85}123\u{85}",
        "\u{b}123\u{b}",
        "1e2",
        "1.5",
        "-1",
        "Infinity",
        "NaN",
        "0x50",
        "0b1010000",
        "0o120",
        "1_000",
        "+42",
    ];
    let largest = usize::MAX.to_string();
    let beyond = (u128::try_from(usize::MAX).unwrap() + 1).to_string();
    let mut cases: Vec<(String, (usize, usize))> =
        accepted.map(|(text, size)| (text.to_owned(), size)).into();
    cases.extend(rejected.map(|text| (text.to_owned(), defaults)));
    cases.push((largest, both(usize::MAX)));
    cases.push((beyond, defaults));
    cases
}

#[test]
fn native_dimensions_accept_only_positive_decimal_environment_values() {
    for (case, (value, expected)) in environment_cases().into_iter().enumerate() {
        isolated!(case, &[("COLUMNS", &value), ("LINES", &value)], || {
            let rig = Rig::paused();
            let _output = pipe_output();
            let terminal = ProcessTerminal::new(rig.local());
            assert_eq!((terminal.columns(), terminal.rows()), expected, "{value:?}");
        });
    }
}

#[test]
fn maestro_stdin_falls_back_to_columns_and_lines_before_default_dimensions() {
    isolated!(0, &[], || {
        let rig = Rig::paused();
        let _output = pipe_output();
        let terminal = ProcessTerminal::new(rig.local());
        assert_eq!((terminal.columns(), terminal.rows()), (80, 24));
    });
    isolated!(1, &[("COLUMNS", "123"), ("LINES", "45")], || {
        let rig = Rig::paused();
        let sizes: [(Option<Native>, (usize, usize)); 5] = [
            (None, (123, 45)),
            (Some((91, 91)), (91, 91)),
            (Some((0, 0)), (123, 45)),
            (Some((91, 0)), (91, 45)),
            (Some((0, 33)), (123, 33)),
        ];
        for (native, expected) in sizes {
            let _output = native.map_or_else(pipe_output, |(columns, rows)| {
                let output = pty_output();
                output.resize(columns, rows);
                output
            });
            let terminal = ProcessTerminal::new(rig.local());
            let size = (terminal.columns(), terminal.rows());
            assert_eq!(size, expected, "{native:?}");
        }
    });
}

/// How the write log is configured in one scenario.
#[derive(Clone, Copy, Debug)]
enum LogCase {
    /// The variable is not set.
    Unset,
    /// The variable is set to the empty string.
    Empty,
    /// The variable names an existing file.
    File,
    /// The variable names an existing directory.
    Directory,
    /// The variable names a file whose parent directory does not exist.
    MissingParent,
    /// The variable names a file below a regular file.
    Blocked,
}

impl LogCase {
    /// Every configuration.
    const ALL: [Self; 6] = [
        Self::Unset,
        Self::Empty,
        Self::File,
        Self::Directory,
        Self::MissingParent,
        Self::Blocked,
    ];

    /// Prepares `dir` and returns the value of the variable, if it is set.
    fn prepare(self, dir: &Path) -> Option<String> {
        let target = match self {
            Self::Unset => return None,
            Self::Empty => return Some(String::new()),
            Self::File => {
                fs::write(dir.join("log.txt"), "old").unwrap();
                dir.join("log.txt")
            }
            Self::Directory => dir.to_owned(),
            Self::MissingParent => dir.join("missing").join("child.log"),
            Self::Blocked => {
                fs::write(dir.join("file"), "kept").unwrap();
                dir.join("file").join("child.log")
            }
        };
        Some(target.to_str().unwrap().to_owned())
    }

    /// Checks what the scenario left in `dir`.
    fn verify(self, dir: &Path) {
        let names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        match self {
            Self::Unset | Self::Empty | Self::MissingParent => {
                assert!(names.is_empty(), "{self:?}");
            }
            Self::File => {
                assert_eq!(
                    fs::read_to_string(dir.join("log.txt")).unwrap(),
                    "oldfirstsecond"
                );
            }
            Self::Directory => {
                let [name] = names.as_slice() else {
                    panic!("one log file, found {names:?}")
                };
                let log = Path::new(name).extension().is_some_and(|ext| ext == "log");
                assert!(name.starts_with("tui-") && log, "{name}");
                assert_eq!(fs::read_to_string(dir.join(name)).unwrap(), "firstsecond");
            }
            Self::Blocked => {
                assert_eq!(names, ["file"]);
                assert_eq!(fs::read_to_string(dir.join("file")).unwrap(), "kept");
            }
        }
    }
}

/// A fresh empty directory for one scenario, made by the parent only.
fn scratch(name: &str, case: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "maestro-terminal-{name}-{case}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes around other output and checks that everything reached standard output.
fn write_around_other_output() {
    let rig = Rig::paused();
    let _feed = pipe_input();
    let output = pipe_output();
    let mut terminal = ProcessTerminal::new(rig.local());
    terminal.write("first").unwrap();
    terminal.hide_cursor().unwrap();
    terminal.set_title("t").unwrap();
    terminal.set_progress(false).unwrap();
    rig.run(async {
        let (inputs, resizes) = (Inputs::default(), Resizes::default());
        terminal
            .start(inputs.callback(), resizes.callback())
            .unwrap();
        terminal.stop().unwrap();
    });
    terminal.write("second").unwrap();
    let expected =
        format!("first\x1b[?25l\x1b]0;t\x07{PROGRESS_CLEAR}{START_BYTES}\x1b[?2004lsecond");
    assert_eq!(output.take(), expected);
}

#[test]
fn native_write_logging_appends_only_explicit_writes() {
    for (case, kind) in LogCase::ALL.into_iter().enumerate() {
        let prepared = (!is_child()).then(|| {
            let dir = scratch("logging", case);
            let target = kind.prepare(&dir);
            (dir, target)
        });
        let target = prepared.as_ref().and_then(|(_, target)| target.as_deref());
        let environment: Vec<(&str, &str)> = target
            .map(|path| ("MAESTRO_TUI_WRITE_LOG", path))
            .into_iter()
            .collect();
        isolated!(case, &environment, write_around_other_output);
        if let Some((dir, _)) = prepared {
            kind.verify(&dir);
            fs::remove_dir_all(&dir).unwrap();
        }
    }
}

#[test]
fn native_log_names_use_local_calendar_fields_and_process_id() {
    for (case, offset_hours) in [0_i64, 14].into_iter().enumerate() {
        let prepared = (!is_child()).then(|| scratch("log-names", case));
        let dir = prepared
            .as_ref()
            .and_then(|dir| dir.to_str())
            .unwrap_or_default();
        let zone = format!("XXX{:+}", -offset_hours);
        let environment = [("MAESTRO_TUI_WRITE_LOG", dir), ("TZ", zone.as_str())];
        isolated!(case, &environment, || {
            let rig = Rig::paused();
            let _output = pipe_output();
            let local =
                chrono::FixedOffset::east_opt(i32::try_from(offset_hours * 3600).unwrap()).unwrap();
            let before = chrono::Utc::now().with_timezone(&local);
            let mut terminal = ProcessTerminal::new(rig.local());
            let after = chrono::Utc::now().with_timezone(&local);
            terminal.write("first").unwrap();
            terminal.write("second").unwrap();
            let directory = std::env::var("MAESTRO_TUI_WRITE_LOG").unwrap();
            let names: Vec<String> = fs::read_dir(&directory)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect();
            let expected = [before, after].map(|time| {
                format!(
                    "tui-{}-{}.log",
                    time.format("%Y-%m-%d_%H-%M-%S"),
                    std::process::id()
                )
            });
            assert!(
                names.len() == 1 && expected.contains(&names[0]),
                "{names:?} against {expected:?}"
            );
            let content = fs::read_to_string(Path::new(&directory).join(&names[0])).unwrap();
            assert_eq!(content, "firstsecond");
        });
        if let Some(dir) = prepared {
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
