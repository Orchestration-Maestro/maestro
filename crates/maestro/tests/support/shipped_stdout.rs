//! Observations of the shipped component stdout routing that hold on every native host.
use super::Root;
use serde_json::json;
use std::{fs::File, process::Command};

/// Initialization output deliberately contains escape and non-text bytes.
pub const INIT: &[u8] = b"INIT\0\xff\x1b[0m";
/// Bytes captured only from the native execution's standard output.
pub const EXEC_STDOUT: &[u8] = b"native-out\0\x1b[32m";
/// Bytes captured only from the native execution's standard error.
pub const EXEC_STDERR: &[u8] = b"native-error\0\x1b[31m";
/// The complete-host-frame opening marker.
pub const FRAME_START: &[u8] = b"\x1b[?2026h";
/// The complete-host-frame closing marker.
pub const FRAME_END: &[u8] = b"\x1b[?2026l";

/// The environment value that makes the shipped guest write `INIT` on `generation` at startup.
pub fn startup_output(generation: &str) -> String {
    json!({"generation": generation, "bytes": INIT}).to_string()
}

/// Noninteractive modes keep inherited bytes both in pipes and in regular files.
pub fn controls(root: &Root, configured: &Command) {
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

/// Removes complete host frames and cursor controls while keeping the guest's SGR bytes.
pub fn without_host_frames(wire: &[u8]) -> Vec<u8> {
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
