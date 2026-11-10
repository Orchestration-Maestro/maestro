//! A recorded stand-in for the remote client that extension tests launch.
#![cfg(unix)]

use std::io::Seek;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use maestro_test_directory::Scratch;
use maestro_test_process::install_family;

/// The largest body the stand-in accepts, in bytes.
const LIMIT: u64 = 4;

/// The record the three behaviours share: one refused destination, the body limit and the
/// remote session's directory.
fn record(session: &Path) -> String {
    serde_json::json!({
        "refusal": {
            "destination": "refused.invalid",
            "code": 255,
            "stderr": "refused destination\n",
        },
        "write": {"limit": LIMIT, "code": 1, "stderr": "remote write too large\n"},
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

/// A body above the limit is refused before it is read, so the remote write never starts.
#[test]
fn stand_in_refuses_a_body_past_its_limit_without_reading_it() {
    let (root, session, alias) = prepared();
    let body = root.path().join("body");
    std::fs::write(&body, b"past the limit").unwrap();
    let mut input = std::fs::File::open(&body).unwrap();

    let output = Command::new(&alias)
        .args(["accepted.invalid", "cat > written"])
        .stdin(Stdio::from(input.try_clone().unwrap()))
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"remote write too large\n");
    assert_eq!(input.stream_position().unwrap(), 0);
    assert!(!session.join("written").exists());
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
