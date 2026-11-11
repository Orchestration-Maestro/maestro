//! Suspension to the background through the bound key, in an isolated child that leads
//! its own session, so the stop signal reaches only that child and a helper it starts.
//! The child's process group is orphaned, so the kernel discards the stop itself while a
//! helper that handles the signal still receives it.
#![cfg(target_os = "linux")]
use super::{World, install, isolated, isolated_output, started};
use maestro_test_process::IsolatedSetup;
use maestro_tui::{Component, interface::InputHandler};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Lines, Write},
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{ChildStdin, ChildStdout, Command, Output, Stdio},
};
use tokio::signal::unix::SignalKind;

/// The default key of `app.suspend`.
const CTRL_Z: &str = "\x1a";

/// Runs `body` on a started mode inside a Tokio runtime with signal delivery, in a child
/// that leads a new session, and returns the child's output to the parent.
fn suspended(test: &str, body: impl AsyncFnOnce(&World, &Path)) -> Option<Output> {
    isolated_output(
        test,
        "child",
        &[],
        IsolatedSetup::new().new_session(),
        async |directory: &Path| {
            let world = started(directory).await;
            body(&world, directory).await;
        },
    )
}

/// Sends `signal` to this process through the native `kill` command, naming the calling
/// thread, and waits for the command.
///
/// Linux queues the signal for the whole process and hands it to the named thread only
/// when that thread can take it at once, so another thread may still be running the
/// signal's action when this returns.
fn signal(signal: &str) {
    let thread = std::fs::read_link("/proc/thread-self").unwrap();
    let thread = thread.file_name().unwrap().to_str().unwrap().to_owned();
    let status = Command::new("kill")
        .args([signal, &thread])
        .status()
        .unwrap();
    assert!(status.success());
}

/// Interrupts this process through [`signal`] and returns once the runtime receives the
/// interrupt: by then every interrupt action registered before the runtime's own has run
/// and left the process alive.
///
/// Signal actions run in registration order and the runtime registers its own the first
/// time this runs, so a caller interrupts only after its first suspension has prepared
/// the suspension's action. An interrupt that takes its default action ends the process
/// inside that earlier action, so this call never returns.
async fn interrupt() {
    let mut interrupts = tokio::signal::unix::signal(SignalKind::interrupt()).unwrap();
    signal("-INT");
    interrupts.recv().await.unwrap();
}

/// What a child printed after each `PROBE ` marker, which can follow the harness's own
/// text on its line.
fn probes(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once("PROBE ").map(|(_, probe)| probe.to_owned()))
        .collect()
}

/// A program in this process's group that reports a received stop signal, then exits.
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
    fn start(directory: &Path) -> Self {
        let member = install(
            directory,
            "member",
            "chat",
            &json!({"scenario": "group-member"}),
        );
        let mut child = Command::new(member)
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

    /// The member's report once it exits after the stop the caller expects: `TSTP`.
    ///
    /// The member's own report decides its exit, so no input is given first.
    fn finish_after_stop(mut self) -> Option<String> {
        let report = self.reports.next().map(Result::unwrap);
        assert!(self.child.wait().unwrap().success());
        report
    }

    /// The member's report once input ends it: nothing, since it received no stop.
    fn finish_by_input(mut self) -> Option<String> {
        if let Some(mut input) = self.input.take() {
            let _ = input.write_all(b"\n");
        }
        self.finish_after_stop()
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
            assert!(shown.contains("Windows has no suspend to background"));
            assert_eq!(world.terminal.events(), ["start"]);
            assert_eq!(world.runtime.futures(), 0);
        },
    );
}

#[test]
fn suspend_resume_restores_the_ui_once() {
    let output = suspended(
        "suspend_resume_restores_the_ui_once",
        async |world, directory| {
            let member = GroupMember::start(directory);
            world.mode.editor.handle_input(CTRL_Z);
            assert_eq!(world.terminal.events(), ["start", "stop"]);
            assert_eq!(member.finish_after_stop().as_deref(), Some("TSTP"));
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
        },
    );
    if let Some(output) = output {
        assert!(output.status.success(), "{output:?}");
    }
}

#[test]
fn suspend_failure_releases_both_temporary_registrations() {
    let output = suspended(
        "suspend_failure_releases_both_temporary_registrations",
        async |world, directory| {
            let member = GroupMember::start(directory);
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
            assert_eq!(member.finish_by_input(), None);
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
        async |world, _| {
            for round in 1..=2 {
                world.mode.editor.handle_input(CTRL_Z);
                interrupt().await;
                println!("PROBE ignored {round}");
                let mut resumed = world.runtime.take_futures();
                signal("-CONT");
                resumed.pop().unwrap().await.unwrap();
            }
            interrupt().await;
            println!("PROBE survived");
        },
    );
    if let Some(output) = output {
        assert_eq!(output.status.signal(), Some(2), "{output:?}");
        assert_eq!(probes(&output), ["ignored 1", "ignored 2"], "{output:?}");
    }
}
