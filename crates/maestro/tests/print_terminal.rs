//! Print mode output as a terminal and as a pipe receive it.
#![cfg(unix)]
use crate::component_stdout::terminal_support::pty;
use crate::support::{Root, provider::Provider};
use std::{
    io::Read,
    process::{Command, Output, Stdio},
};

/// What a model can write to act on a terminal: a clipboard write, a window title, a screen
/// clear and a link whose label disguises its target.
const HOSTILE: &str = "see \x1b]52;c;aGk=\x07the \x1b]0;owned\x1b\\docs \x1b[2Jat \
    \x1b]8;;https://evil.example\x1b\\example.org\x1b]8;;\x1b\\";

/// The print command for `mode` against a provider that answers `HOSTILE`.
fn print(root: &Root, mode: &str) -> Command {
    let mut command = root.command(&[
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--no-session",
        "--mode",
        mode,
        "-p",
        "write the page",
    ]);
    command.stdin(Stdio::null()).stderr(Stdio::piped());
    command
}

/// Runs `mode` with standard output on a pseudo-terminal and returns what the terminal showed
/// and the process's streams once it exited.
fn on_terminal(mode: &str) -> (String, Output) {
    let root = Root::new();
    let provider = Provider::replying(HOSTILE);
    root.models(&provider.base);
    let (master, slave) = pty();
    let mut command = print(&root, mode);
    command.stdout(Stdio::from(slave));
    let child = command.spawn().unwrap();
    // The terminal reports its end only once no copy of its slave side stays open.
    drop(command);
    let mut shown = Vec::new();
    match std::fs::File::from(master).read_to_end(&mut shown) {
        Ok(_) => {}
        Err(error) if error.raw_os_error() == Some(rustix::io::Errno::IO.raw_os_error()) => {}
        Err(error) => panic!("{error}"),
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    (String::from_utf8(shown).unwrap(), output)
}

/// A terminal shows the words a model wrote and none of the sequences that act on it.
#[test]
fn text_on_a_terminal_loses_its_control_sequences() {
    let (shown, _) = on_terminal("text");
    assert_eq!(shown, "see the docs at example.org\r\n");
}

/// A pipe or a file keeps the model's bytes exactly, so scripts see what it wrote.
#[test]
fn text_on_a_pipe_stays_byte_exact() {
    let root = Root::new();
    let provider = Provider::replying(HOSTILE);
    root.models(&provider.base);
    let output = print(&root, "text")
        .stdout(Stdio::piped())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(output.stdout, format!("{HOSTILE}\n").into_bytes());
}

/// JSON records escape their controls, so a terminal never receives one raw.
#[test]
fn json_on_a_terminal_keeps_its_controls_escaped() {
    let (shown, _) = on_terminal("json");
    assert!(!shown.contains('\x1b'), "{shown:?}");
    assert!(shown.contains(r"see \u001b]52;c;aGk=\u0007the \u001b]0;owned"));
}
