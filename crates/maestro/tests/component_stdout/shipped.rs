//! Shipped stdout routing with a real terminal input and an unread output pipe.
use crate::support::{ChildGuard, Root};
use maestro_test_process::{HARNESS_START, TestProcess, child};
use serde_json::json;
use std::{
    fs::File,
    io::{BufReader, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    process::{ChildStdout, Command, Stdio},
};

/// Initialization output deliberately contains escape and non-text bytes.
const INIT: &[u8] = b"INIT\0\xff\x1b[0m";
/// The complete-host-frame opening marker.
const FRAME_START: &[u8] = b"\x1b[?2026h";
/// The complete-host-frame closing marker.
const FRAME_END: &[u8] = b"\x1b[?2026l";

/// Runs both standard output generations through actual executable startup and reload.
pub(super) fn run() {
    for generation in ["p2", "p3"] {
        let root = Root::new();
        root.models("http://127.0.0.1:1/v1");
        let helper = TestProcess::child(acknowledge).unwrap().command().unwrap();
        let component = std::env::var("MAESTRO_WASI_COMPONENT").unwrap();
        let mut command = root.command(&["--offline", "--extension", &component]);
        command
            .env("MAESTRO_WASI_ROOT_OUTPUT", root.path("ack"))
            .env(
                "MAESTRO_WASI_ROOT_COMMAND",
                json!({
                    "command":helper.get_program().to_str().unwrap(),
                    "args":helper.get_args().map(|arg| arg.to_str().unwrap()).collect::<Vec<_>>(),
                    "prologue":HARNESS_START,
                })
                .to_string(),
            )
            .env(
                "MAESTRO_WASI_STARTUP_OUTPUT",
                json!({
                    "generation":generation,"bytes":INIT,
                })
                .to_string(),
            );
        controls(&root, &command);
        interactive(&root, &mut command, generation);
    }
}

/// Noninteractive modes keep inherited bytes both in pipes and in regular files.
fn controls(root: &Root, configured: &Command) {
    for mode in ["text", "json", "rpc"] {
        let mut command = root.command(&[]);
        command
            .args(configured.get_args())
            .args(["--print", "--mode", mode]);
        command.envs(
            configured
                .get_envs()
                .filter_map(|(key, value)| value.map(|value| (key, value))),
        );
        let output = command.output().unwrap();
        assert!(output.status.success(), "{mode}: {output:?}");
        assert!(output.stdout.starts_with(INIT), "{mode}: {output:?}");
        assert!(!output.stderr.windows(INIT.len()).any(|bytes| bytes == INIT));
        let path = root.path("stdout");
        let destination = File::create(&path).unwrap();
        let output = command.stdout(destination).output().unwrap();
        assert!(output.status.success(), "{mode}: {output:?}");
        assert!(std::fs::read(path).unwrap().starts_with(INIT), "{mode}");
    }
}

/// Waits on output markers and child acknowledgements, never elapsed time.
fn interactive(root: &Root, command: &mut Command, generation: &str) {
    let (master, slave) = super::terminal_support::pty();
    let mut input = File::from(master);
    let listener = UnixListener::bind(root.path("ack")).unwrap();
    let mut process = command
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
            json!({"terminal":false})
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

/// Removes complete host frames and cursor controls while keeping the guest's SGR bytes.
fn without_host_frames(wire: &[u8]) -> Vec<u8> {
    let mut guest = Vec::new();
    let mut rest = wire;
    while !rest.is_empty() {
        if let Some(body) = rest.strip_prefix(FRAME_START) {
            let end = body
                .windows(FRAME_END.len())
                .position(|bytes| bytes == FRAME_END)
                .unwrap();
            assert!(
                !body[..end]
                    .windows(b"guest\0\xff".len())
                    .any(|bytes| bytes == b"guest\0\xff"),
                "guest output split a host frame"
            );
            rest = &body[end + FRAME_END.len()..];
        } else if let Some(control) = rest.strip_prefix(b"\x1b[") {
            let end = control
                .iter()
                .position(|byte| (0x40..=0x7e).contains(byte))
                .unwrap();
            let sequence = &rest[..end + 3];
            if sequence == b"\x1b[36m" || sequence == b"\x1b[0m" {
                guest.extend_from_slice(sequence);
            }
            rest = &rest[end + 3..];
        } else {
            guest.push(rest[0]);
            rest = &rest[1..];
        }
    }
    guest
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
