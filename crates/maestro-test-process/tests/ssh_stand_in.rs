//! A recorded stand-in for the remote client that extension tests launch.
#![cfg(unix)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use maestro_test_directory::Scratch;
use maestro_test_process::install_family;

/// The body the refused write carries, past every pipe buffer.
const BODY: usize = 2 * 1024 * 1024;

/// The record the three behaviours share: one refused destination, the refused write and the
/// remote session's directory.
fn record(session: &Path) -> String {
    serde_json::json!({
        "refusal": {
            "destination": "refused.invalid",
            "code": 255,
            "stderr": "refused destination\n",
        },
        "write": {
            "command": "early-write.txt",
            "code": 38,
            "stderr": "remote write rejected\n",
        },
        "directory": session,
    })
    .to_string()
}

/// Install the prepared artifact as `ssh` beside that record.
fn stand_in(root: &Path, session: &Path) -> PathBuf {
    let alias = root.join(format!("ssh{}", std::env::consts::EXE_SUFFIX));
    install_family(&alias, "ssh", &record(session)).unwrap();
    alias
}

/// A scratch root with the session directory the record names.
fn prepared() -> (Scratch, PathBuf, PathBuf) {
    let root = Scratch::new().unwrap();
    let session = root.path().join("session");
    std::fs::create_dir(&session).unwrap();
    let alias = stand_in(root.path(), &session);
    (root, session, alias)
}

/// The recorded destination is refused with its status and diagnostic, on standard error alone.
#[test]
fn stand_in_refuses_the_recorded_destination_with_its_status_and_diagnostic() {
    let (_root, _session, alias) = prepared();

    let output = Command::new(&alias)
        .args(["refused.invalid", "true"])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(255),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"refused destination\n");
}

/// The recorded write is refused, so a body streamed into it is never read and never written.
#[test]
fn stand_in_refuses_a_recorded_write_without_reading_its_body() {
    let (_root, session, alias) = prepared();

    let mut child = Command::new(&alias)
        .args(["accepted.invalid", "cat > early-write.txt"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let streamed = input.write_all(&vec![b'x'; BODY]);
    drop(input);
    let output = child.wait_with_output().unwrap();

    assert_eq!(
        output.status.code(),
        Some(38),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"remote write rejected\n");
    assert!(
        streamed.is_err(),
        "the stand-in read the body instead of refusing it"
    );
    assert!(!session.join("early-write.txt").exists());
}

/// A launch left with no destination is refused with the family's own status, silently.
#[test]
fn stand_in_refuses_a_launch_without_a_destination() {
    let (_root, _session, alias) = prepared();

    let output = Command::new(&alias).arg("-v").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

/// A record naming a field the family does not define is rejected before any effect.
#[test]
fn stand_in_rejects_a_record_naming_an_unknown_field() {
    let root = Scratch::new().unwrap();
    let session = root.path().join("session");
    std::fs::create_dir(&session).unwrap();
    let mut record: serde_json::Value = serde_json::from_str(&record(&session)).unwrap();
    record["body_length"] = serde_json::json!(4);
    let alias = root.path().join("ssh");
    install_family(&alias, "ssh", &record.to_string()).unwrap();

    let output = Command::new(&alias)
        .args(["refused.invalid", "true"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown field"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Every other command runs through the real shell in the recorded session directory.
#[test]
fn stand_in_runs_other_commands_through_the_shell_in_the_recorded_directory() {
    let (_root, session, alias) = prepared();
    std::fs::write(session.join("note"), b"remote body\n").unwrap();

    let output = Command::new(&alias)
        .args([
            "accepted.invalid",
            "pwd; cat note; test -f note && echo present; mkdir made",
        ])
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let printed = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(lines[0], session.display().to_string());
    assert_eq!(lines[1..], ["remote body", "present"]);
    assert!(session.join("made").is_dir());
}
