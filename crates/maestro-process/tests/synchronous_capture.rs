//! Synchronous capture through the public native process interface.
#![cfg(test)]
#![cfg(unix)]
use crate::child_programs::{self, async_command, harness};
use maestro_process::{OutputMode, SpawnSyncOptions, spawn_sync};
use maestro_test_process::HARNESS_START;
use std::{os::unix::process::ExitStatusExt, time::Duration};
use tokio::process::Command;

#[test]
fn sync_capture_keeps_raw_stdout_status_and_closed_input() {
    let directory = std::env::temp_dir().canonicalize().unwrap();
    for code in [0, 7] {
        let mut command = async_command(
            child_programs::capture_probe,
            &[("CHILD_EXIT", &code.to_string())],
        );
        command
            .args(["literal $HOME;|", "", "literal $HOME;|"])
            .env("MAESTRO_CAPTURE", "chosen")
            .current_dir(&directory);
        let output = spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                input: None,
                timeout: Some(Duration::from_secs(5)),
                stderr: OutputMode::Discard,
            },
        )
        .unwrap();
        let mut expected = format!(
            "literal $HOME;|||literal $HOME;||chosen|{}|True|True",
            directory.display()
        )
        .into_bytes();
        expected.extend_from_slice(b"\0\xff");
        assert_eq!(output.stdout, harness(&expected));
        assert!(output.stderr.is_empty());
        assert_eq!(output.status.code(), Some(code));
    }
    assert_signal_status();
}

/// Checks that synchronous capture retains native signal status.
fn assert_signal_status() {
    let mut signal = async_command(child_programs::self_terminating, &[]);
    let output = spawn_sync(
        &mut signal,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: Some(Duration::from_secs(5)),
            stderr: OutputMode::Discard,
        },
    )
    .unwrap();
    assert_eq!(output.status.signal(), Some(15));
    assert_eq!(output.stdout, harness(b""));
}

#[test]
fn sync_capture_is_callable_inside_a_running_runtime() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut command = Command::new("printf");
            command.arg("nested");
            assert_eq!(
                spawn_sync(
                    &mut command,
                    SpawnSyncOptions::Capture {
                        max_buffer_bytes: None,
                        stdout: OutputMode::Capture,
                        input: None,
                        timeout: Some(Duration::from_secs(5)),
                        stderr: OutputMode::Discard
                    }
                )
                .unwrap()
                .stdout,
                b"nested"
            );
            maestro_process::spawn(Command::new("true"), None)
                .unwrap()
                .join()
                .unwrap();
        });
}

#[test]
fn sync_capture_distinguishes_spawn_failure_from_completed_failure() {
    use maestro_process::SpawnSyncError;
    use std::{error::Error, io};
    let missing = std::env::temp_dir().join(format!("maestro-missing-{}", std::process::id()));
    for (path, kind) in [
        (missing, io::ErrorKind::NotFound),
        (std::env::temp_dir(), io::ErrorKind::PermissionDenied),
    ] {
        let native = std::process::Command::new(&path).spawn().unwrap_err();
        let error = spawn_sync(
            &mut Command::new(&path),
            SpawnSyncOptions::Capture {
                max_buffer_bytes: None,
                stdout: OutputMode::Capture,
                input: None,
                timeout: Some(Duration::from_secs(5)),
                stderr: OutputMode::Discard,
            },
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "Cannot start the process");
        let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
        assert_eq!(source.raw_os_error(), native.raw_os_error());
        assert!(matches!(error, SpawnSyncError::Spawn(ref e) if e.kind() == kind));
    }
    let mut command = Command::new("sh");
    command.args(["-c", "printf done; exit 7"]);
    let output = spawn_sync(
        &mut command,
        SpawnSyncOptions::Capture {
            max_buffer_bytes: None,
            stdout: OutputMode::Capture,
            input: None,
            timeout: Some(Duration::from_secs(5)),
            stderr: OutputMode::Discard,
        },
    )
    .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"done");
}

/// Source-observed combined buffer boundaries.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Boundary {
    /// Bytes emitted to stdout.
    stdout: usize,
    /// Bytes emitted to stderr.
    stderr: usize,
    /// Selected combined limit, zero disabling it.
    max_buffer: usize,
    /// Whether collection completes normally.
    ok: bool,
}

#[test]
fn bounded_capture_counts_combined_output() {
    let rows: Vec<Boundary> =
        serde_json::from_str(include_str!("fixtures/capture_boundaries.json")).unwrap();
    for Boundary {
        stdout: out,
        stderr: err,
        max_buffer,
        ok: succeeds,
    } in rows
    {
        // The harness start bytes count toward the combined limit; a zero limit stays off.
        let limit = Some(if max_buffer == 0 {
            0
        } else {
            max_buffer + HARNESS_START.len()
        });
        let mut command = async_command(
            child_programs::sized_writer,
            &[
                ("CHILD_OUT", &out.to_string()),
                ("CHILD_ERR", &err.to_string()),
            ],
        );
        let result = spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: None,
                timeout: None,
                max_buffer_bytes: limit,
            },
        );
        if succeeds {
            let output = result.unwrap();
            assert_eq!(output.stdout, harness(&vec![b'x'; out]));
            assert_eq!(output.stderr, vec![b'y'; err]);
        } else {
            assert!(matches!(
                result,
                Err(maestro_process::SpawnSyncError::Wait(_))
            ));
        }
    }
}

#[test]
fn bounded_capture_reaps_overflowing_child() {
    use std::{
        io::{Read as _, Write as _},
        net::TcpListener,
    };
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = std::thread::spawn(move || {
        let mut command = async_command(
            child_programs::overflowing_writer,
            &[("CHILD_PORT", &port.to_string())],
        );
        spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: None,
                timeout: None,
                max_buffer_bytes: Some(16),
            },
        )
    });
    let (mut socket, _) = listener.accept().unwrap();
    let mut pid = Vec::new();
    loop {
        let mut byte = [0];
        socket.read_exact(&mut byte).unwrap();
        if byte[0] == b'\n' {
            break;
        }
        pid.push(byte[0]);
    }
    socket.write_all(b"g").unwrap();
    assert!(matches!(
        worker.join().unwrap(),
        Err(maestro_process::SpawnSyncError::Wait(_))
    ));
    let pid = String::from_utf8(pid).unwrap().parse().unwrap();
    assert_eq!(
        rustix::process::test_kill_process(rustix::process::Pid::from_raw(pid).unwrap()),
        Err(rustix::io::Errno::SRCH)
    );
    let mut rest = Vec::new();
    let _ = socket.read_to_end(&mut rest);
    assert!(rest.is_empty());
}
