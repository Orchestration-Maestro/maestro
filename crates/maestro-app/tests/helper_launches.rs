//! Application exports use the original native launch interfaces.
#![cfg(unix)]
use maestro_app::{ExecOptions, ExecResult};
use maestro_process::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn, spawn_sync};
use maestro_test_directory::Scratch;
use maestro_tools::exec_command;
use serde_json::json;
use tokio::process::Command;

/// One controlled program reporting `record`.
fn program(
    directory: &std::path::Path,
    name: &str,
    record: &serde_json::Value,
) -> std::path::PathBuf {
    crate::native::command(directory, name, record)
}

#[test]
fn application_launches_use_the_shared_process_interfaces() {
    let scratch = Scratch::new().unwrap();
    let selected = program(
        scratch.path(),
        "launch",
        &json!({"scenario": "output", "stdout": b"out", "stderr": b"err", "code": 7}),
    );
    let quiet = program(
        scratch.path(),
        "quiet",
        &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 0}),
    );
    let mut command = Command::new(&selected);
    command.args(["literal operand", "", "a b"]);
    let options: maestro_process::SpawnSyncOptions<'_> = SpawnSyncOptions::Capture {
        max_buffer_bytes: None,
        stdout: OutputMode::Capture,
        input: None,
        timeout: None,
        stderr: OutputMode::Capture,
    };
    let result: Result<std::process::Output, SpawnSyncError> = spawn_sync(&mut command, options);
    let result: Result<std::process::Output, maestro_process::SpawnSyncError> = result;
    let output = result.unwrap();
    assert_eq!(output.stdout, b"out");
    assert_eq!(output.stderr, b"err");
    assert_eq!(output.status.code(), Some(7));
    let output = spawn_sync(&mut Command::new(&quiet), SpawnSyncOptions::Inherit).unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    spawn(Command::new(&quiet), None).unwrap().join().unwrap();
    let runtime = crate::support::io_runtime();
    let options: maestro_tools::ExecOptions<'_> = ExecOptions::default();
    let output: ExecResult = runtime
        .block_on(exec_command(
            selected.to_str().unwrap(),
            &["literal operand".into()],
            ".",
            options,
        ))
        .unwrap();
    let output: maestro_tools::ExecResult = output;
    assert_eq!(output.stdout, "out");
    assert_eq!(output.stderr, "err");
    assert_eq!(output.code, 7);
    assert!(!output.killed);
}
