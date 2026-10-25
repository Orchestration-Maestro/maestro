//! Application exports use the original native launch interfaces.
#![cfg(unix)]
use maestro_app::{
    ExecOptions, ExecResult, OutputMode, SpawnSyncError, SpawnSyncOptions, exec_command, spawn,
    spawn_sync,
};
use tokio::process::Command;

#[test]
fn application_launches_use_the_shared_process_interfaces() {
    let mut command = Command::new("sh");
    command.args(["-c", "printf out; printf err >&2; exit 7"]);
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
    let output = spawn_sync(&mut Command::new("true"), SpawnSyncOptions::Inherit).unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    spawn(Command::new("true"), None).unwrap().join().unwrap();
    let runtime = crate::support::io_runtime();
    let options: maestro_tools::process::exec::ExecOptions<'_> = ExecOptions::default();
    let output: ExecResult = runtime
        .block_on(exec_command(
            "sh",
            &[
                "-c".into(),
                "printf async-out; printf async-err >&2; exit 7".into(),
            ],
            ".",
            options,
        ))
        .unwrap();
    let output: maestro_tools::process::exec::ExecResult = output;
    assert_eq!(output.stdout, "async-out");
    assert_eq!(output.stderr, "async-err");
    assert_eq!(output.code, 7);
    assert!(!output.killed);
}
