//! Suspension to the background through the bound key, in an isolated child that leads
//! its own session, so the stop signal reaches only that child and a helper it starts.
//! The child's process group is orphaned, so the kernel discards the stop itself while a
//! helper that handles the signal still receives it.
#![cfg(target_os = "linux")]
use super::{World, isolated, isolated_output, started};
use maestro_tui::{Component, tui::InputHandler};
use std::{
    io::{BufRead, BufReader, Lines, Write},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{ChildStdin, ChildStdout, Command, Output, Stdio},
};

/// The default key of `app.suspend`.
const CTRL_Z: &str = "\x1a";

/// Runs `body` on a started mode inside a Tokio runtime with signal delivery, in a child
/// that leads a new session, and returns the child's output to the parent.
fn suspended(test: &str, body: impl AsyncFnOnce(&World)) -> Option<Output> {
    isolated_output(
        test,
        "child",
        &[],
        "exec setsid \"$@\"",
        async |directory: &Path| {
            let world = started(directory).await;
            body(&world).await;
        },
    )
}

/// Sends `signal` to this process through the native `kill` command, naming the calling
/// thread: Linux then delivers it to that thread, which runs the signal's action before
/// the command's wait returns, so the next statement observes its outcome.
fn signal(signal: &str) {
    let thread = std::fs::read_link("/proc/thread-self").unwrap();
    let thread = thread.file_name().unwrap().to_str().unwrap().to_owned();
    let status = Command::new("kill")
        .args([signal, &thread])
        .status()
        .unwrap();
    assert!(status.success());
}

/// What a child printed after each `PROBE ` marker, which can follow the harness's own
/// text on its line.
fn probes(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once("PROBE ").map(|(_, probe)| probe.to_owned()))
        .collect()
}

/// A shell in this process's group that reports a received stop signal, then exits.
struct GroupMember {
    /// Held open until the member should stop waiting.
    input: Option<ChildStdin>,
    /// The member's reports.
    reports: Lines<BufReader<ChildStdout>>,
    /// The member.
    child: std::process::Child,
}

impl GroupMember {
    /// Starts the member and waits until it handles the stop signal.
    fn start() -> Self {
        let mut child = Command::new("bash")
            .args(["-c", "trap 'echo TSTP; exit' TSTP; echo ready; read -r _"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut reports = BufReader::new(child.stdout.take().unwrap()).lines();
        assert_eq!(reports.next().unwrap().unwrap(), "ready");
        let input = child.stdin.take();
        Self {
            input,
            reports,
            child,
        }
    }

    /// The member's report once it exits: `TSTP`, or nothing when it received no stop.
    fn finish(mut self) -> Option<String> {
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(b"\n");
        }
        let report = self.reports.next().map(Result::unwrap);
        assert!(self.child.wait().unwrap().success());
        report
    }
}

#[test]
fn suspend_windows_reports_without_native_effects() {
    isolated(
        "suspend_windows_reports_without_native_effects",
        async |directory| {
            let world = started(directory).await;
            world.mode.show_suspend_unsupported().unwrap();
            let shown = world.mode.chat_container.render(80).concat();
            assert!(shown.contains("Suspend to background is not supported on Windows"));
            assert_eq!(world.terminal.events(), ["start"]);
            assert_eq!(world.runtime.futures(), 0);
        },
    );
}

#[test]
fn suspend_resume_restores_the_ui_once() {
    let output = suspended("suspend_resume_restores_the_ui_once", async |world| {
        let member = GroupMember::start();
        world.mode.editor.handle_input(CTRL_Z);
        assert_eq!(world.terminal.events(), ["start", "stop"]);
        assert_eq!(member.finish().as_deref(), Some("TSTP"));
        let mut resumed = world.runtime.take_futures();
        assert_eq!(resumed.len(), 1);
        world.terminal.clear_writes();
        signal("-CONT");
        resumed.pop().unwrap().await.unwrap();
        assert_eq!(world.terminal.events(), ["start", "stop", "start"]);
        world.runtime.settle().unwrap();
        let border = world.mode.editor.render(80).swap_remove(0);
        assert!(world.terminal.writes().concat().contains(&border));
        signal("-CONT");
        world.runtime.settle().unwrap();
        assert_eq!(world.runtime.futures(), 0);
        assert_eq!(world.terminal.events(), ["start", "stop", "start"]);
    });
    if let Some(output) = output {
        assert!(output.status.success(), "{output:?}");
    }
}

#[test]
fn suspend_failure_releases_both_temporary_registrations() {
    let output = suspended(
        "suspend_failure_releases_both_temporary_registrations",
        async |world| {
            let member = GroupMember::start();
            world
                .terminal
                .set_stop_error(Some(std::io::ErrorKind::Other));
            world.mode.editor.handle_input(CTRL_Z);
            assert_eq!(world.terminal.events(), ["start", "stop"]);
            let mut reported = world.runtime.take_futures();
            assert_eq!(reported.len(), 1);
            let error = reported.pop().unwrap().await.unwrap_err();
            assert_eq!(error.to_string(), "stop failure");
            signal("-CONT");
            world.runtime.settle().unwrap();
            assert_eq!(world.runtime.futures(), 0);
            assert_eq!(world.terminal.events(), ["start", "stop"]);
            assert_eq!(member.finish(), None);
            println!("PROBE released");
            signal("-INT");
        },
    );
    if let Some(output) = output {
        assert_eq!(output.status.signal(), Some(2), "{output:?}");
        assert_eq!(probes(&output), ["released"], "{output:?}");
    }
}

#[test]
fn suspend_preserves_existing_interrupt_ownership() {
    let output = suspended(
        "suspend_preserves_existing_interrupt_ownership",
        async |world| {
            for round in 1..=2 {
                world.mode.editor.handle_input(CTRL_Z);
                signal("-INT");
                println!("PROBE ignored {round}");
                let mut resumed = world.runtime.take_futures();
                signal("-CONT");
                resumed.pop().unwrap().await.unwrap();
            }
            signal("-INT");
            println!("PROBE survived");
        },
    );
    if let Some(output) = output {
        assert_eq!(output.status.signal(), Some(2), "{output:?}");
        assert_eq!(probes(&output), ["ignored 1", "ignored 2"], "{output:?}");
    }
}
