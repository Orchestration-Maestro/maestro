//! Shipped component stdout routing on native Windows: the inherited-output modes and the
//! interactive startup on a console.
//!
//! The Linux and macOS cases drive a pseudo-terminal that suspends the executable with
//! `SIGTSTP` and `SIGCONT`. Windows has no terminal stop signal, and the console cannot be
//! fed input by these harnesses, so suspend and resume, reload and the probes that need typed
//! input are not qualified here; the interactive startup and its drain are.
#![cfg(windows)]
use crate::support::{
    Root,
    shipped_stdout::{INIT, controls, startup_output, without_host_frames},
};
use crate::windows_terminal::console_input;
use maestro_test_process::on_console;
use std::process::Command;

/// The executable with the shipped component, its startup output on `generation`, and a
/// controlled model it never reaches.
fn shipped(root: &Root, generation: &str) -> Command {
    root.models("http://127.0.0.1:1/v1");
    let component = std::env::var("MAESTRO_WASI_COMPONENT").unwrap();
    let mut command = root.command(&["--offline", "--extension", &component]);
    command.env("MAESTRO_WASI_STARTUP_OUTPUT", startup_output(generation));
    command
}

/// Print, JSON and RPC modes keep the guest's `generation` bytes in pipes and in files.
fn inherited_output(generation: &str) {
    let root = Root::new();
    controls(&root, &shipped(&root, generation));
}

/// The first output generation keeps its inherited bytes in every noninteractive mode.
#[test]
fn p2_inherited_output_stays_raw_in_every_noninteractive_mode() {
    inherited_output("p2");
}

/// The second output generation keeps its inherited bytes in every noninteractive mode.
#[test]
fn p3_inherited_output_stays_raw_in_every_noninteractive_mode() {
    inherited_output("p3");
}

/// Starts the executable on the console, stops it at once and checks where the guest's
/// startup bytes landed relative to the terminal's start, its host frames and its stop.
fn startup_on_a_console(generation: &str) {
    let root = Root::new();
    let mut command = shipped(&root, generation);
    let output = console_input(&mut command)
        .env("MAESTRO_STARTUP_BENCHMARK", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let wire = output.stdout;
    let position = |marker: &[u8]| wire.windows(marker.len()).position(|bytes| bytes == marker);
    assert_eq!(position(INIT), Some(0), "{wire:?}");
    let (started, stopped) = (position(b"\x1b[?2004h"), position(b"\x1b[?2004l"));
    assert!(started > Some(INIT.len()) && stopped > started, "{wire:?}");
    assert!(without_host_frames(&wire).starts_with(INIT));
}

/// The first output generation's startup bytes reach the console outside host frames.
#[test]
fn p2_startup_output_reaches_a_console_run_outside_host_frames() {
    on_console(
        "p2_startup_output_reaches_a_console_run_outside_host_frames",
        |command| command,
        || startup_on_a_console("p2"),
    )
    .unwrap();
}

/// The second output generation's startup bytes reach the console outside host frames.
#[test]
fn p3_startup_output_reaches_a_console_run_outside_host_frames() {
    on_console(
        "p3_startup_output_reaches_a_console_run_outside_host_frames",
        |command| command,
        || startup_on_a_console("p3"),
    )
    .unwrap();
}
