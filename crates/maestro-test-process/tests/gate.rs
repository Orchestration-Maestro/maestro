//! Gates that hold a native child until the test releases it.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Stdio};

use maestro_test_directory::Scratch;
use maestro_test_process::gate::Gate;
use maestro_test_process::install_family;

/// Install a `command` alias that announces on `ready`, waits on `permit`, then prints `tail`.
fn held_child(root: &Path, ready: &Gate, permit: &Gate) -> std::process::Child {
    let record = serde_json::json!({
        "scenario": "hold",
        "ready": ready.path(),
        "permit": permit.path(),
        "stdout": b"tail",
        "stderr": [],
    });
    let entry = root.join("held");
    install_family(&entry, "command", &record.to_string()).unwrap();
    Command::new(entry).stdout(Stdio::piped()).spawn().unwrap()
}

/// One release lets exactly one held child finish with its recorded output.
#[test]
fn gate_release_lets_a_held_child_finish() {
    let root = Scratch::new().unwrap();
    let ready = Gate::new(&root.path().join("ready")).unwrap();
    let permit = Gate::new(&root.path().join("permit")).unwrap();
    let child = held_child(root.path(), &ready, &permit);
    assert_eq!(ready.take(5).unwrap(), b"ready");
    permit.release().unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"tail");
}

/// A failed FIFO creation never opens or removes an existing file at the gate path.
#[test]
fn gate_creation_preserves_an_existing_file() {
    let root = Scratch::new().unwrap();
    let path = root.path().join("existing gate");
    std::fs::write(&path, b"keep").unwrap();

    assert!(Gate::new(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"keep");
}

/// A gate dropped before its release ends the held child instead of leaving it blocked.
#[test]
fn gate_drop_ends_a_held_child() {
    let root = Scratch::new().unwrap();
    let ready = Gate::new(&root.path().join("ready")).unwrap();
    let permit = Gate::new(&root.path().join("permit")).unwrap();
    let child = held_child(root.path(), &ready, &permit);
    assert_eq!(ready.take(5).unwrap(), b"ready");
    drop(permit);
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
