//! The executable's interactive mode and selectors on a Windows console.
//!
//! Each case needs a console as standard input, so the test starts a copy of itself on a
//! new console, and that copy starts the executable with the console input buffer as its
//! standard input and a pipe as its standard output.
#![cfg(windows)]
use crate::support::Root;
use maestro_test_process::TestProcess;
use std::{
    fs::OpenOptions,
    io::Read,
    os::windows::process::CommandExt,
    process::{Command, Stdio},
};

/// Marks the copy that runs on the new console.
const ROLE: &str = "MAESTRO_ROOT_TEST_CONSOLE";

/// What the copy writes to standard error when its case finished.
const DONE: &str = "console case finished";

/// Gives a child process a console of its own.
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Runs `case` in a copy of this test that owns a new console.
fn on_console(test: &str, case: impl FnOnce()) {
    if std::env::var_os(ROLE).is_some() {
        case();
        eprintln!("{DONE}");
        return;
    }
    let output = TestProcess::current(test)
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|arguments| {
            Command::new(std::env::current_exe()?)
                .args(arguments)
                .env(ROLE, "1")
                .creation_flags(CREATE_NEW_CONSOLE)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
        })
        .unwrap();
    let report = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{test} failed on its console:\n{report}"
    );
    assert!(report.contains(DONE), "{test} did not run:\n{report}");
}

/// Gives `command` this process's console as standard input.
fn console_input(command: &mut Command) -> &mut Command {
    let console = OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .unwrap();
    command.stdin(console)
}

/// Starts the executable with `args`, reads its output until `marker`, then ends it.
fn presents(root: &Root, args: &[&str], marker: &str) {
    let mut child = console_input(&mut root.command(args))
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let (mut bytes, mut chunk) = (Vec::new(), [0; 4096]);
    while !String::from_utf8_lossy(&bytes).contains(marker) {
        let count = stdout.read(&mut chunk).unwrap();
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
    child.kill().unwrap();
    child.wait().unwrap();
    let output = String::from_utf8_lossy(&bytes);
    assert!(output.contains(marker), "{marker} never shown: {output:?}");
}

#[test]
fn interactive_mode_starts_and_stops_on_a_console() {
    on_console("interactive_mode_starts_and_stops_on_a_console", || {
        let root = Root::new();
        let output = console_input(&mut root.command(&[]))
            .env("MAESTRO_STARTUP_BENCHMARK", "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success(),
            "{stdout:?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("\x1b[?2004h"),
            "terminal never started: {stdout:?}"
        );
        assert!(
            stdout.contains("\x1b[?2004l"),
            "terminal never stopped: {stdout:?}"
        );
    });
}

#[test]
fn configuration_selector_opens_on_a_console() {
    on_console("configuration_selector_opens_on_a_console", || {
        presents(&Root::new(), &["config"], "Resource Configuration");
    });
}

#[test]
fn session_selector_opens_on_a_console() {
    on_console("session_selector_opens_on_a_console", || {
        presents(&Root::new(), &["--resume"], "Resume Session");
    });
}

#[test]
fn missing_directory_selector_opens_on_a_console() {
    on_console("missing_directory_selector_opens_on_a_console", || {
        let root = Root::new();
        let session = root.path("agent/gone.jsonl");
        let header = serde_json::json!({
            "type": "session",
            "version": 3,
            "id": "gone-session",
            "cwd": root.path("gone").display().to_string(),
            "timestamp": "2025-01-01T00:00:00.000Z",
        });
        std::fs::write(&session, format!("{header}\n")).unwrap();
        presents(
            &root,
            &["--session", session.to_str().unwrap()],
            "cwd from session file does not exist",
        );
    });
}
