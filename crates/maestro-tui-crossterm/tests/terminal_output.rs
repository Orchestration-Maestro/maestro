//! Process terminal output: exact bytes, progress, dimensions and the write log. Each
//! scenario runs in a child process that owns its standard descriptors and environment.
#![cfg(test)]
#![cfg(unix)]

use crate::{isolated, support};

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use crate::support::{
    Capture, Inputs, Resizes, Rig, START_BYTES, elapse, is_child, pipe_input, pipe_output,
    pty_output,
};
use maestro_tui::Terminal;
use maestro_tui_crossterm::ProcessTerminal;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::stdio::stdout;

/// An exec child must not retain either terminal pipe after its standard streams change.
#[cfg(target_os = "linux")]
#[test]
fn native_terminal_descriptors_do_not_survive_exec() {
    const PROBE: &str = "MAESTRO_TERMINAL_DESCRIPTOR_PROBE";
    if let Ok(targets) = std::env::var(PROBE) {
        let targets: Vec<_> = targets.lines().map(PathBuf::from).collect();
        for entry in fs::read_dir("/proc/self/fd").unwrap() {
            if let Ok(target) = fs::read_link(entry.unwrap().path()) {
                assert!(
                    !targets.contains(&target),
                    "inherited terminal pipe: {target:?}"
                );
            }
        }
        return;
    }
    isolated!(&[], || {
        use rustix::io::{FdFlags, fcntl_setfd};
        use std::process::Stdio;
        let rig = Rig::paused();
        let input = pipe_input();
        let output = pipe_output();
        fcntl_setfd(&input.0, FdFlags::CLOEXEC).unwrap();
        fcntl_setfd(&output.0, FdFlags::CLOEXEC).unwrap();
        let targets =
            ["/proc/self/fd/0", "/proc/self/fd/1"].map(|path| fs::read_link(path).unwrap());
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let child = maestro_test_process::TestProcess::current(
                "native_terminal_descriptors_do_not_survive_exec",
            )
            .unwrap()
            .env(
                PROBE,
                format!("{}\n{}", targets[0].display(), targets[1].display()),
            )
            .stdin(Stdio::null())
            .output()
            .unwrap();
            terminal.stop().unwrap();
            assert!(
                child.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            );
        });
    });
}

/// Collects exact bytes, including scalars split across kernel reads.
fn collect_bytes(capture: Capture, length: usize) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        use std::io::Read;
        let flags = fcntl_getfl(&capture.0).unwrap();
        fcntl_setfl(&capture.0, flags & !OFlags::NONBLOCK).unwrap();
        let mut file = std::fs::File::from(capture.0);
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes).unwrap();
        bytes
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
        let collected = collect_bytes(capture, data.len());
        terminal.write(&data).unwrap();
        let received = collected.join().unwrap();
        assert_eq!(received.len(), data.len());
        assert!(received.iter().all(|byte| *byte == b'x'));
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
            output.written(PROGRESS_ACTIVE).await;
            elapse(999).await;
            assert_eq!(output.take(), "");
            terminal.set_progress(true).unwrap();
            output.written(PROGRESS_ACTIVE).await;
            elapse(1).await;
            output.written(PROGRESS_ACTIVE).await;
            elapse(1000).await;
            output.written(PROGRESS_ACTIVE).await;
            terminal.set_progress(false).unwrap();
            assert_eq!(output.take(), PROGRESS_CLEAR);
            elapse(2001).await;
            assert_eq!(output.take(), "");
        });
    });
}

#[test]
fn native_stop_clears_active_progress_before_input_modes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _feed = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let (inputs, resizes) = (Inputs::default(), Resizes::default());
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal
                .start(inputs.callback(), resizes.callback())
                .unwrap();
            output.written(START_BYTES).await;
            terminal.set_progress(true).unwrap();
            output.written(PROGRESS_ACTIVE).await;
            terminal.stop().unwrap();
            assert_eq!(output.take(), format!("{PROGRESS_CLEAR}\x1b[?2004l\x1b[<u"));
            elapse(3001).await;
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

/// Environment values and the dimensions they select when standard output reports none: one
/// value of each kind of number text that is accepted, and of the kinds that are refused.
fn environment_cases() -> Vec<(&'static str, (usize, usize))> {
    let defaults = (80, 24);
    let both = |value: usize| (value, value);
    vec![
        ("123", both(123)),
        ("00080", both(80)),
        ("+42", both(42)),
        ("1e2", both(100)),
        ("0x50", both(80)),
        ("0b1010000", both(80)),
        ("0o120", both(80)),
        ("42.0", both(42)),
        ("\u{feff}\u{b}123\u{a0}\u{2028}", both(123)),
        ("", defaults),
        ("0", defaults),
        ("-1", defaults),
        ("1.5", defaults),
        ("Infinity", defaults),
        ("\u{85}123", defaults),
        ("1_000", defaults),
        ("12 3", defaults),
    ]
}

#[test]
fn native_dimensions_use_the_numeric_value_of_environment_text() {
    for (case, (value, expected)) in environment_cases().into_iter().enumerate() {
        isolated!(case, &[("COLUMNS", value), ("LINES", value)], || {
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
fn scratch() -> (maestro_test_directory::Scratch, PathBuf) {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let dir = scratch.path().to_path_buf();
    (scratch, dir)
}

/// Writes around other output and checks that everything reached standard output, and that a
/// write standard output refuses is not logged.
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
    let expected = format!(
        "first\x1b[?25l\x1b]0;t\x07{PROGRESS_CLEAR}{START_BYTES}\x1b[?25h\x1b[?2004l\x1b[<usecond"
    );
    assert_eq!(output.take(), expected);
    drop(output);
    let failure = terminal.write("lost").unwrap_err();
    assert_eq!(failure.kind(), std::io::ErrorKind::BrokenPipe);
}

#[test]
fn native_write_logging_appends_only_explicit_writes() {
    for (case, kind) in LogCase::ALL.into_iter().enumerate() {
        let prepared = (!is_child()).then(|| {
            let (root, dir) = scratch();
            let target = kind.prepare(&dir);
            (root, dir, target)
        });
        let target = prepared
            .as_ref()
            .and_then(|(_, _, target)| target.as_deref());
        let environment: Vec<(&str, &str)> = target
            .map(|path| ("MAESTRO_TUI_WRITE_LOG", path))
            .into_iter()
            .collect();
        isolated!(case, &environment, write_around_other_output);
        if let Some((_, dir, _)) = prepared {
            kind.verify(&dir);
        }
    }
}

#[test]
fn native_log_names_use_local_calendar_fields_and_process_id() {
    for (case, offset_hours) in [0_i64, 14].into_iter().enumerate() {
        let prepared = (!is_child()).then(scratch);
        let dir = prepared
            .as_ref()
            .and_then(|(_, dir)| dir.to_str())
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
        drop(prepared);
    }
}

#[test]
fn native_output_backlog_keeps_input_responsive() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let output = pty_output();
        let input = pipe_input();
        rig.run(async {
            let (sent, received) = tokio::sync::oneshot::channel();
            let mut sent = Some(sent);
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal
                .start(
                    Box::new(move |_| {
                        if let Some(sent) = sent.take() {
                            sent.send(()).unwrap();
                        }
                    }),
                    Box::new(|| {}),
                )
                .unwrap();
            output.written(START_BYTES).await;
            let data = "x".repeat(2 * 1024 * 1024);
            terminal.write(&data).unwrap();
            input.send(b"a");
            received.await.unwrap();
            assert!(terminal.pending_output_bytes() > 0);
            let _reader = rustix::io::dup(&output.0).unwrap();
            let collector = collect_bytes(output, data.len());
            while !collector.is_finished() {
                tokio::task::yield_now().await;
            }
            assert_eq!(collector.join().unwrap(), data.as_bytes());
            terminal.stop().unwrap();
        });
    });
}

/// Starts over the caller's installed input, consuming only startup output.
async fn queued_terminal(rig: &Rig, output: &Capture) -> ProcessTerminal {
    let mut terminal = ProcessTerminal::new(rig.local());
    terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
    output.written(START_BYTES).await;
    terminal
}

#[test]
fn native_output_preserves_repeated_chunk_order() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            terminal.write("first").unwrap();
            terminal.show_cursor().unwrap();
            terminal.write("second").unwrap();
            terminal.write("first").unwrap();
            output.written("first\x1b[?25hsecondfirst").await;
            assert_eq!(terminal.pending_output_bytes(), 0);
            terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_output_retries_after_descriptor_backpressure() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            fcntl_setfl(stdout(), fcntl_getfl(stdout()).unwrap() | OFlags::NONBLOCK).unwrap();
            let data = "r".repeat(2 * 1024 * 1024);
            terminal.write(&data).unwrap();
            support::until(|| terminal.pending_output_bytes() < data.len()).await;
            assert!(terminal.pending_output_bytes() > 0);
            let _reader = rustix::io::dup(&output.0).unwrap();
            let collector = collect_bytes(output, data.len());
            support::until(|| collector.is_finished()).await;
            assert_eq!(collector.join().unwrap(), data.as_bytes());
            assert_eq!(terminal.pending_output_bytes(), 0);
            terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_output_counts_every_partial_drain() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let data = "p".repeat(2 * 1024 * 1024);
            terminal.write(&data).unwrap();
            assert_eq!(terminal.pending_output_bytes(), data.len());
            support::until(|| terminal.pending_output_bytes() < data.len()).await;
            let remaining = terminal.pending_output_bytes();
            assert!(remaining > 0);
            let first = output.take();
            assert_eq!(first.len() + remaining, data.len());
            support::until(|| terminal.pending_output_bytes() < remaining).await;
            let later = terminal.pending_output_bytes();
            assert!(later > 0 && later < remaining);
            let _reader = rustix::io::dup(&output.0).unwrap();
            let collector = collect_bytes(output, remaining);
            support::until(|| collector.is_finished()).await;
            assert_eq!(collector.join().unwrap(), vec![b'p'; remaining]);
            assert_eq!(terminal.pending_output_bytes(), 0);
            terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_output_small_writes_keep_exact_accounting() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let _reader = rustix::io::dup(&output.0).unwrap();
            let collector = collect_bytes(output, 100_000);
            for accepted in 1..=100_000 {
                terminal.write("s").unwrap();
                assert_eq!(terminal.pending_output_bytes(), accepted);
            }
            support::until(|| collector.is_finished()).await;
            assert_eq!(collector.join().unwrap(), vec![b's'; 100_000]);
            assert_eq!(terminal.pending_output_bytes(), 0);
            terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_output_failure_drops_pending_and_surfaces_at_stop() {
    isolated!(0, &[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            terminal.write(&"f".repeat(2 * 1024 * 1024)).unwrap();
            drop(output);
            support::until(|| terminal.pending_output_bytes() == 0).await;
            terminal.write("discarded").unwrap();
            assert_eq!(terminal.pending_output_bytes(), 0);
            assert_eq!(
                terminal.stop().unwrap_err().kind(),
                std::io::ErrorKind::BrokenPipe
            );
        });
    });

    isolated!(1, &[], || {
        let rig = Rig::paused();
        let directory = fs::File::open("/").unwrap();
        rustix::stdio::dup2_stdin(&directory).unwrap();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
            let acquired = support::open_descriptors();
            support::until(|| support::open_descriptors() < acquired).await;
            drop(output);
            terminal.write("after-input-failure").unwrap();
            support::until(|| terminal.pending_output_bytes() == 0).await;
            assert_eq!(
                terminal.stop().unwrap_err().kind(),
                std::io::ErrorKind::IsADirectory
            );
        });
    });
}

#[test]
fn native_output_stop_and_drop_drain_before_modes() {
    for case in 0..2 {
        isolated!(case, &[], || {
            let rig = Rig::paused();
            let _input = pipe_input();
            let output = pipe_output();
            rig.run(async {
                let mut terminal = queued_terminal(&rig, &output).await;
                let data = "d".repeat(2 * 1024 * 1024);
                terminal.write(&data).unwrap();
                support::until(|| terminal.pending_output_bytes() < data.len()).await;
                assert!(terminal.pending_output_bytes() > 0);
                terminal.write("tail").unwrap();
                terminal.set_progress(true).unwrap();
                let expected =
                    format!("{data}tail{PROGRESS_ACTIVE}{PROGRESS_CLEAR}\x1b[?2004l\x1b[<u");
                let collector = collect_bytes(output, expected.len());
                if case == 0 {
                    terminal.stop().unwrap();
                } else {
                    drop(terminal);
                }
                assert_eq!(collector.join().unwrap(), expected.as_bytes());
            });
        });
    }
}

#[test]
fn native_output_session_bytes_stay_identical() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = ProcessTerminal::new(rig.local());
            terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
            output.written(START_BYTES).await;
            terminal.write("first🦀first").unwrap();
            terminal.move_by(0).unwrap();
            terminal.move_by(-2).unwrap();
            terminal.clear_line().unwrap();
            terminal.clear_from_cursor().unwrap();
            terminal.clear_screen().unwrap();
            terminal.set_title("x\x07\x1b]0;y").unwrap();
            terminal.set_progress(true).unwrap();
            terminal.set_progress(false).unwrap();
            terminal.stop().unwrap();
            assert_eq!(output.take(), "first🦀first\x1b[2A\x1b[K\x1b[J\x1b[2J\x1b[H\x1b]0;x\x07\x1b]0;y\x07\x1b]9;4;3\x07\x1b]9;4;0;\x07\x1b[?2004l\x1b[<u");
        });
    });
}

#[test]
fn native_output_regular_destinations_remain_synchronous() {
    for case in 0..2 {
        isolated!(case, &[], || {
            use rustix::stdio::dup2_stdout;
            use std::fs::File;
            let rig = Rig::paused();
            let _input = pipe_input();
            let path = std::env::temp_dir().join(format!("maestro-output-{}", std::process::id()));
            let file = if case == 0 {
                File::options()
                    .create_new(true)
                    .read(true)
                    .write(true)
                    .open(&path)
                    .unwrap()
            } else {
                File::options()
                    .read(true)
                    .write(true)
                    .open("/dev/null")
                    .unwrap()
            };
            dup2_stdout(&file).unwrap();
            rig.run(async {
                let mut terminal = ProcessTerminal::new(rig.local());
                terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
                terminal.write("regular").unwrap();
                assert_eq!(terminal.pending_output_bytes(), 0);
                if case == 0 {
                    assert_eq!(
                        fs::read_to_string(&path).unwrap(),
                        format!("{START_BYTES}regular")
                    );
                }
                terminal.stop().unwrap();
            });
            if case == 0 {
                fs::remove_file(path).unwrap();
            }
        });
    }
}

#[test]
fn native_output_restores_output_flags() {
    for case in 0..8 {
        isolated!(case, &[], || {
            let rig = Rig::paused();
            let _input = pipe_input();
            let output = if case >= 4 {
                pty_output()
            } else {
                pipe_output()
            };
            if case >= 4 {
                rustix::stdio::dup2_stdin(stdout()).unwrap();
            }
            let original = support::stdout_flags();
            let flags = if case % 2 == 0 {
                original
            } else {
                original | OFlags::NONBLOCK
            };
            fcntl_setfl(stdout(), flags).unwrap();
            rig.run(async {
                let mut terminal = queued_terminal(&rig, &output).await;
                assert!(fcntl_getfl(stdout()).unwrap().contains(OFlags::NONBLOCK));
                terminal.write("flags").unwrap();
                // Some hosts' drain of a terminal waits until its reader takes this output.
                let reader = Capture(output.0.try_clone().unwrap());
                let collector = collect_bytes(reader, "flags\x1b[?2004l\x1b[<u".len());
                if case % 4 < 2 {
                    terminal.stop().unwrap();
                } else {
                    drop(terminal);
                }
                assert_eq!(support::stdout_flags(), flags);
                assert_eq!(collector.join().unwrap(), b"flags\x1b[?2004l\x1b[<u");
            });
        });
    }

    isolated!(8, &[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let file = fs::File::open("/dev/null").unwrap();
        rustix::stdio::dup2_stdout(&file).unwrap();
        let flags = fcntl_getfl(stdout()).unwrap();
        let input_flags = fcntl_getfl(rustix::stdio::stdin()).unwrap();
        rig.run(async {
            let mut terminal = ProcessTerminal::new(rig.local());
            let baseline = support::open_descriptors();
            assert!(terminal.start(Box::new(|_| {}), Box::new(|| {})).is_err());
            assert_eq!(fcntl_getfl(stdout()).unwrap(), flags);
            assert_eq!(fcntl_getfl(rustix::stdio::stdin()).unwrap(), input_flags);
            assert_eq!(support::open_descriptors(), baseline);
        });
    });
}

#[test]
fn native_output_restart_keeps_pending_bytes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let data = "o".repeat(2 * 1024 * 1024);
            terminal.write(&data).unwrap();
            support::until(|| terminal.pending_output_bytes() < data.len()).await;
            assert!(terminal.pending_output_bytes() > 0);
            terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
            terminal.write("new").unwrap();
            let expected = format!("{data}\x1b[?2004h\x1b[?u\x1b[cnew\x1b[?2004l\x1b[<u");
            let collector = collect_bytes(output, expected.len());
            terminal.stop().unwrap();
            assert_eq!(collector.join().unwrap(), expected.as_bytes());
        });
    });
}

#[test]
fn native_output_empty_and_multibyte_counts_are_bytes() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            terminal.write("").unwrap();
            assert_eq!(terminal.pending_output_bytes(), 0);
            let data = format!("{}🦀", "a".repeat(16383));
            terminal.write(&data).unwrap();
            assert_eq!(terminal.pending_output_bytes(), 16387);
            let _reader = rustix::io::dup(&output.0).unwrap();
            let collector = collect_bytes(output, data.len());
            support::until(|| collector.is_finished()).await;
            assert_eq!(collector.join().unwrap(), data.as_bytes());
            terminal.stop().unwrap();
        });
    });
}

#[test]
fn native_output_log_records_accepted_explicit_writes() {
    let prepared = (!is_child()).then(scratch);
    let path = prepared.as_ref().map(|(_, dir)| dir.join("write.log"));
    let configured = path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let environment: Vec<_> = configured
        .as_deref()
        .map(|value| ("MAESTRO_TUI_WRITE_LOG", value))
        .into_iter()
        .collect();
    isolated!(&environment, || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            terminal.write("explicit").unwrap();
            terminal.set_title("unlogged").unwrap();
            let path = std::env::var_os("MAESTRO_TUI_WRITE_LOG").unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), "explicit");
            assert!(terminal.pending_output_bytes() > 0);
            terminal.stop().unwrap();
            assert_eq!(
                output.take(),
                "explicit\x1b]0;unlogged\x07\x1b[?2004l\x1b[<u"
            );
            drop(output);
            assert!(terminal.write("refused").is_err());
            assert_eq!(fs::read_to_string(path).unwrap(), "explicit");
        });
    });
    if let Some(dir) = prepared {
        drop(dir);
    }
}

/// Requires the actual returned writer to be transferable without sharing the local queue.
fn send_sync<T: Send + Sync>(_: &T) {}

#[test]
fn native_output_writer_orders_cross_thread_bytes() {
    isolated!(&[], || {
        use std::io::Write;
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let mut writer = terminal.output_writer().unwrap();
            send_sync(&writer);
            let mut foreign = writer.clone();
            terminal.write("frame-a").unwrap();
            std::thread::spawn(move || {
                assert_eq!(foreign.write(&[]).unwrap(), 0);
                foreign.write_all(b"\0\xff\x1b[31m").unwrap();
                foreign.flush().unwrap();
            })
            .join()
            .unwrap();
            terminal.write("frame-b").unwrap();
            writer.write_all(b"tail").unwrap();
            let expected = b"frame-a\0\xff\x1b[31mframe-btail\x1b[?2004l\x1b[<u";
            let collector = collect_bytes(output, expected.len());
            terminal.stop().unwrap();
            assert_eq!(collector.join().unwrap(), expected);
        });
    });
}

#[test]
fn native_output_writer_drains_then_closes_admission() {
    isolated!(0, &[], || {
        use std::io::Write;
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        let flags = support::stdout_flags();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let mut writer = terminal.output_writer().unwrap();
            let data = "a".repeat(2 * 1024 * 1024);
            terminal.write(&data).unwrap();
            writer.write_all(b"\xff\0external").unwrap();
            support::until(|| terminal.pending_output_bytes() < data.len()).await;
            assert!(terminal.pending_output_bytes() > 0);
            let mut expected = data.into_bytes();
            expected.extend_from_slice(b"\xff\0external\x1b[?2004l\x1b[<u");
            let collector = collect_bytes(output, expected.len());
            terminal.stop().unwrap();
            assert_eq!(support::stdout_flags(), flags);
            assert_eq!(collector.join().unwrap(), expected);
            assert!(terminal.output_writer().is_none());
            assert_closed_writer(&mut writer);
            let next = pipe_output();
            terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
            let mut fresh = terminal.output_writer().unwrap();
            fresh.write_all(b"fresh").unwrap();
            assert_eq!(
                writer.write(b"old").unwrap_err().kind(),
                std::io::ErrorKind::BrokenPipe
            );
            terminal.stop().unwrap();
            assert_eq!(next.take(), format!("{START_BYTES}fresh\x1b[?2004l\x1b[<u"));
        });
    });
    isolated!(1, &[], || {
        use std::io::Write;
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        rig.run(async {
            let mut terminal = queued_terminal(&rig, &output).await;
            let mut writer = terminal.output_writer().unwrap();
            drop(output);
            writer.write_all(b"failure").unwrap();
            support::until(|| terminal.pending_output_bytes() == 0).await;
            assert_eq!(
                writer.write(b"lost").unwrap_err().kind(),
                std::io::ErrorKind::BrokenPipe
            );
            assert_eq!(
                terminal.stop().unwrap_err().kind(),
                std::io::ErrorKind::BrokenPipe
            );
        });
    });
}

#[test]
fn native_output_writer_keeps_synchronous_destinations() {
    isolated!(&[], || {
        let rig = Rig::paused();
        let _input = pipe_input();
        let output = pipe_output();
        let mut terminal = ProcessTerminal::new(rig.local());
        assert!(terminal.output_writer().is_none());
        terminal.write("stopped").unwrap();
        assert_eq!(output.take(), "stopped");
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let path = scratch.path().join("regular");
        let file = fs::File::create(&path).unwrap();
        rustix::stdio::dup2_stdout(&file).unwrap();
        rig.run(async {
            terminal.start(Box::new(|_| {}), Box::new(|| {})).unwrap();
            assert!(terminal.output_writer().is_none());
            terminal.write("synchronous").unwrap();
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                format!("{START_BYTES}synchronous")
            );
            terminal.stop().unwrap();
            assert!(terminal.output_writer().is_none());
        });
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            format!("{START_BYTES}synchronous\x1b[?2004l\x1b[<u")
        );
    });
}

/// Verifies closure for ordinary, empty and flush admission without writing more bytes.
fn assert_closed_writer(writer: &mut maestro_tui_crossterm::terminal::output::OutputWriter) {
    use std::io::Write;
    assert_eq!(
        writer.write(b"lost").unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        writer.write(&[]).unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
    assert_eq!(
        writer.flush().unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
}
