//! Shipped stdout routing with a real terminal input and an unread output pipe.
use crate::support::{
    ChildGuard, Root,
    shipped_stdout::{FRAME_END, INIT, controls, startup_output, without_host_frames},
};
use maestro_test_process::{HARNESS_START, TestProcess, child};
use rustix::process::{Pid, Signal, WaitOptions, kill_process, waitpid};
use serde_json::json;
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    os::unix::{
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    process::{ChildStdout, Command, Stdio},
};

/// Bytes captured only from the native execution's standard output.
const EXEC_STDOUT: &[u8] = b"native-out\0\x1b[32m";
/// Bytes captured only from the native execution's standard error.
const EXEC_STDERR: &[u8] = b"native-error\0\x1b[31m";

/// Runs both output generations through executable startup, suspension and reload.
pub(super) fn run() {
    for generation in ["p2", "p3"] {
        let root = Root::new();
        root.models("http://127.0.0.1:1/v1");
        let helper = TestProcess::child(acknowledge).unwrap().command().unwrap();
        let execution = TestProcess::child(native_exec).unwrap().command().unwrap();
        let component = std::env::var("MAESTRO_WASI_COMPONENT").unwrap();
        let mut command = root.command(&["--offline", "--extension", &component]);
        command
            .env("MAESTRO_WASI_ROOT_OUTPUT", root.path("ack"))
            .env(
                "MAESTRO_WASI_ROOT_COMMAND",
                json!({
                    "command":helper.get_program().to_str().unwrap(),
                    "args":helper.get_args().map(|arg| arg.to_str().unwrap()).collect::<Vec<_>>(),
                    "exec_args":execution.get_args().map(|arg| arg.to_str().unwrap()).collect::<Vec<_>>(),
                    "prologue":HARNESS_START,
                })
                .to_string(),
            )
            .env("MAESTRO_WASI_STARTUP_OUTPUT", startup_output(generation));
        controls(&root, &command);
        interactive(&root, &mut command, generation);
    }
}

/// Waits on output markers and child acknowledgements, never elapsed time.
fn interactive(root: &Root, command: &mut Command, generation: &str) {
    let (master, slave) = super::terminal_support::pty();
    let mut input = File::from(master);
    let listener = UnixListener::bind(root.path("ack")).unwrap();
    // A separate group in our session lets the kernel honor its terminal stop signal.
    let mut process = command
        .process_group(0)
        .stdin(Stdio::from(slave))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(process.stdout.take().unwrap());
    let mut errors = process.stderr.take().unwrap();
    let guard = ChildGuard(&mut process);
    let mut wire = Vec::new();
    through(&mut output, &mut wire, b"ROOT-STDOUT-READY\x07");
    through(&mut output, &mut wire, FRAME_END);
    assert!(wire.windows(INIT.len()).any(|bytes| bytes == INIT));
    suspend(Pid::from_child(guard.0), &mut input, &mut output, &mut wire);
    probe(&mut input, &listener, &mut output, &mut wire, generation);
    input.write_all(b"/quit\r").unwrap();
    output.read_to_end(&mut wire).unwrap();
    let status = guard.0.wait().unwrap();
    let mut stderr = String::new();
    errors.read_to_string(&mut stderr).unwrap();
    assert!(status.success(), "{stderr}");
    assert!(
        wire.windows(b"\x1b[?2004l".len())
            .any(|bytes| bytes == b"\x1b[?2004l")
    );
    without_host_frames(&wire);
}

/// Observes the real group stop before continuing the executable and waiting for its redraw.
fn suspend(pid: Pid, input: &mut File, output: &mut BufReader<ChildStdout>, wire: &mut Vec<u8>) {
    input.write_all(b"\x1a").unwrap();
    let (stopped, status) = waitpid(Some(pid), WaitOptions::UNTRACED).unwrap().unwrap();
    assert_eq!(stopped, pid);
    assert_eq!(status.stopping_signal(), Some(Signal::TSTP.as_raw()));
    through(output, wire, b"\x1b[?2004l");
    kill_process(pid, Signal::CONT).unwrap();
    through(output, wire, b"\x1b[?2004h");
    through(output, wire, FRAME_END);
}

/// Repeats the guest's exact-byte write on both sides of a real extension reload.
fn probe(
    input: &mut File,
    listener: &UnixListener,
    output: &mut BufReader<ChildStdout>,
    wire: &mut Vec<u8>,
    generation: &str,
) {
    let payload = [
        b"\x1b[36m".as_slice(),
        &b"guest\0\xff".repeat(300_000),
        b"\x1b[0m",
    ]
    .concat();
    for attempt in 1..=2 {
        input
            .write_all(format!("/stdout-probe {generation}\r").as_bytes())
            .unwrap();
        // Keep stdout unread until native execution acknowledges the guest's write attempt.
        let (mut peer, _) = listener.accept().unwrap();
        let mut reply = Vec::new();
        peer.read_to_end(&mut reply).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&reply).unwrap(),
            json!({
                "output":{"terminal":false},
                "exec":{
                    "stdout":String::from_utf8([HARNESS_START, EXEC_STDOUT].concat()).unwrap(),
                    "stderr":String::from_utf8(EXEC_STDERR.to_vec()).unwrap(),
                    "code":0,
                    "killed":false,
                },
            })
        );
        through(output, wire, b"ROOT-STDOUT-DONE\x07");
        let guest = without_host_frames(wire);
        assert_eq!(occurrences(&guest, &payload), attempt);
        if attempt == 1 {
            input.write_all(b"/stdout-probe reload\r").unwrap();
            through(output, wire, b"ROOT-STDOUT-READY\x07");
            through(
                output,
                wire,
                b"Reloaded keybindings, extensions, skills, prompts, themes",
            );
            through(output, wire, FRAME_END);
            assert_eq!(
                wire.windows(INIT.len())
                    .filter(|bytes| *bytes == INIT)
                    .count(),
                2
            );
        }
    }
}

/// Reads a bounded marker from the stream while retaining the exact observed bytes.
fn through(output: &mut BufReader<ChildStdout>, wire: &mut Vec<u8>, marker: &[u8]) {
    loop {
        let mut byte = [0];
        assert_eq!(output.read(&mut byte).unwrap(), 1, "EOF before {marker:?}");
        wire.push(byte[0]);
        if wire.ends_with(marker) {
            return;
        }
    }
}

/// Counts nonoverlapping complete payloads without comparing every repeated interior prefix.
fn occurrences(mut bytes: &[u8], payload: &[u8]) -> usize {
    let mut count = 0;
    while let Some(at) = bytes
        .windows(payload.len())
        .position(|window| window == payload)
    {
        count += 1;
        bytes = &bytes[at + payload.len()..];
    }
    count
}

/// A real native execution produces distinct streams without the terminal's stdout route.
#[test]
#[ignore = "runs only inside a controlled child"]
fn native_exec() {
    std::io::stdout().write_all(EXEC_STDOUT).unwrap();
    std::io::stderr().write_all(EXEC_STDERR).unwrap();
    child::leave(0);
}

/// A real native child sends the guest's result only after its stdout attempt completed.
#[test]
#[ignore = "runs only inside a controlled child"]
fn acknowledge() {
    let endpoint = std::env::var("MAESTRO_STDOUT_ENDPOINT").unwrap();
    let reply = std::env::var("MAESTRO_STDOUT_REPLY").unwrap();
    UnixStream::connect(endpoint)
        .unwrap()
        .write_all(reply.as_bytes())
        .unwrap();
    child::leave(0);
}
