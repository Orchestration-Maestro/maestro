//! Captured native children through the public package operations.
#![cfg(test)]
#![cfg(unix)]
use crate::{native_support, scratch};
use maestro_packages::{CommandCaptureOptions, NativePackageOperations, PackageOperations};
use serde_json::{Value, json};
use std::{io, path::Path, rc::Rc, time::Duration};

use crate::runtime::runtime;

/// One installed stand-in and what a capture of it returned.
struct Captured {
    /// The installed alias, whose spelling every failure message names.
    alias: String,
    /// The captured standard output, or the failure a caller asserts.
    result: io::Result<String>,
}

/// Captures one stand-in installed in a scratch directory of its own.
fn captured(name: &str, record: &Value) -> Captured {
    let scratch = scratch::Scratch::new().unwrap();
    capture_in(scratch.path(), name, record, None)
}

/// Captures one stand-in below `directory`, which owns the alias.
fn capture_in(directory: &Path, name: &str, record: &Value, timeout: Option<Duration>) -> Captured {
    let alias = native_support::command(directory, name, record);
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(
        Rc::new(|| panic!("capture queried stdout takeover")),
        &local,
    );
    let result = runtime().block_on(operations.run_command_capture(
        &alias.to_string_lossy(),
        &[],
        CommandCaptureOptions {
            cwd: None,
            timeout,
            env: &[],
        },
    ));
    Captured {
        alias: alias.to_string_lossy().into_owned(),
        result,
    }
}

/// One descendant controlled through native FIFO producer acknowledgements.
struct Producer {
    /// Owns the directory the three pipes live in.
    _scratch: scratch::Scratch,
    /// Marker the stand-in writes before it blocks.
    ready_path: std::path::PathBuf,
    /// Release the stand-in waits for.
    permit: std::path::PathBuf,
    /// Pipe a late-opening reader waits for.
    written: std::path::PathBuf,
    /// Release sent only after the producer's readiness witness.
    release: std::sync::mpsc::Sender<()>,
    /// The joined native FIFO peer.
    thread: std::thread::JoinHandle<()>,
    /// Readiness after the stand-in reaches its holding point.
    ready: tokio::sync::oneshot::Receiver<()>,
}
impl Producer {
    /// Creates native pipes, with a thread that acknowledges and waits for release.
    fn new() -> Self {
        use std::io::{Read, Write};
        let scratch = scratch::Scratch::new().unwrap();
        let (ready_path, permit, written) = Self::pipes(&scratch);
        let (send, receive) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let (ready, released, late) = (ready_path.clone(), permit.clone(), written.clone());
        let thread = std::thread::spawn(move || {
            let mut text = String::new();
            std::fs::File::open(ready)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert_eq!(text, "ready");
            send.send(()).unwrap();
            wait.recv().unwrap();
            // Read-write open never waits for a reader, which a terminated stand-in may never be.
            let mut permit = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(released)
                .unwrap();
            permit.write_all(b"go\n").unwrap();
            let mut written = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(late)
                .unwrap();
            written.write_all(b"x\n").unwrap();
            // Closing the last descriptor discards unread FIFO bytes, so hold it until the capture ends.
            wait.recv().unwrap();
        });
        Self {
            _scratch: scratch,
            ready_path,
            permit,
            written,
            release,
            thread,
            ready: receive,
        }
    }
    /// The three native pipes the fixture's gate names.
    fn pipes(
        scratch: &scratch::Scratch,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let ready = scratch.path().join("ready");
        let permit = scratch.path().join("permit");
        let written = scratch.path().join("written");
        assert!(
            std::process::Command::new("mkfifo")
                .args([&ready, &permit, &written])
                .status()
                .unwrap()
                .success()
        );
        (ready, permit, written)
    }
    /// The gate a stand-in waits behind before it writes `stdout` and `stderr`.
    fn gate(&self, late: bool, stdout: &[u8], stderr: &[u8]) -> Value {
        gate(
            &self.ready_path,
            &self.permit,
            late.then_some(self.written.as_path()),
            stdout,
            stderr,
        )
    }
}

/// One release behind a readiness marker, with the bytes the stand-in reports afterwards.
fn gate(ready: &Path, permit: &Path, late: Option<&Path>, stdout: &[u8], stderr: &[u8]) -> Value {
    json!({
        "scenario": "hold",
        "ready": ready,
        "permit": permit,
        "late": late,
        "stdout": stdout,
        "stderr": stderr
    })
}

#[test]
fn captured_success_uses_stdout_only_and_exact_trim() {
    let bom = captured(
        "trimmed",
        &native_support::output(b"\xEF\xBB\xBF  alpha  beta\n ", b"ignored", 0),
    );
    assert_eq!(bom.result.unwrap(), "alpha  beta");
    assert_eq!(
        captured("diagnostic", &native_support::output(b"", b"diagnostic", 0))
            .result
            .unwrap(),
        ""
    );
    assert_eq!(
        captured("nel", &native_support::output(b"\xC2\x85x\xC2\x85", b"", 0))
            .result
            .unwrap(),
        "\u{85}x\u{85}"
    );
}

#[test]
fn capture_failure_keeps_stream_precedence_and_signal() {
    for (name, record, status, payload) in [
        (
            "failed",
            native_support::output(b"output", b" diagnostic\n", 7),
            "code 7",
            " diagnostic\n",
        ),
        (
            "trimmed-failure",
            native_support::output(b" output\n", b"", 8),
            "code 8",
            " output\n",
        ),
        (
            "blank-diagnostic",
            native_support::output(b"output", b" ", 9),
            "code 9",
            " ",
        ),
        (
            "terminated",
            json!({"scenario": "output", "stdout": [], "stderr": [], "code": 0, "signal": "SIGTERM"}),
            "signal SIGTERM",
            "",
        ),
    ] {
        let capture = captured(name, &record);
        assert_eq!(
            capture.result.unwrap_err().to_string(),
            format!("{}  failed with {status}: {payload}", capture.alias)
        );
    }
    #[cfg(target_os = "linux")]
    {
        let capture = captured(
            "realtime",
            &json!({"scenario": "output", "stdout": [], "stderr": [], "code": 0, "signal": "SIGRTMAX"}),
        );
        assert_eq!(
            capture.result.unwrap_err().to_string(),
            format!("{}  failed with signal unknown: ", capture.alias)
        );
    }
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    assert_eq!(
        runtime()
            .block_on(operations.run_command_capture(
                "/usr/bin/false",
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &[]
                }
            ))
            .unwrap_err()
            .to_string(),
        "/usr/bin/false  failed with code 1: "
    );
}

/// The inherited path the re-executed test binary runs with.
const INHERITED_PATH: &str = "/usr/bin:/bin:/maestro-inherited";

#[test]
fn capture_inherits_environment_and_applies_scoped_overrides() {
    use std::io::Write;
    const NAME: &str = "capture_inherits_environment_and_applies_scoped_overrides";
    if std::env::var_os("MAESTRO_CAPTURE_REEXEC").is_none() {
        let mut child = maestro_test_process::TestProcess::current(NAME)
            .unwrap()
            .single_threaded()
            .env("MAESTRO_CAPTURE_REEXEC", "1")
            .env("MAESTRO_INHERITED", "parent")
            .env("PATH", INHERITED_PATH)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"parent stdin bytes\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{text}");
        return;
    }
    let scratch = scratch::Scratch::new().unwrap();
    let record = json!({
        "scenario": "report",
        "fields": [
            {"variable": "PATH"},
            {"variable": "MAESTRO_INHERITED"},
            {"variable": "MAESTRO_CAPTURE_VALUE"},
            "working-directory",
            {"operand": 0},
            "input",
        ]
    });
    let alias = native_support::command(scratch.path(), "report", &record);
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let cwd = scratch.path().to_string_lossy().into_owned();
    let output = runtime()
        .block_on(operations.run_command_capture(
            &alias.to_string_lossy(),
            &["a; b  c".to_owned()],
            CommandCaptureOptions {
                cwd: Some(&cwd),
                timeout: None,
                env: &[("MAESTRO_CAPTURE_VALUE", "chosen")],
            },
        ))
        .unwrap();
    assert_eq!(
        output,
        format!("{INHERITED_PATH}|parent|chosen|{cwd}|a; b  c|0")
    );
}

#[test]
fn capture_waits_for_exit_and_both_pipe_eofs() {
    let producer = Producer::new();
    let scratch = scratch::Scratch::new().unwrap();
    let descendant = native_support::command(
        scratch.path(),
        "hold",
        &producer.gate(false, b"tail", b"diagnostic"),
    );
    let holder = native_support::command(
        scratch.path(),
        "holder",
        &json!({"scenario": "holder", "stdout": b"early", "stderr": [], "descendant": descendant}),
    );
    let expected = holder.to_string_lossy().into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let runtime = runtime();
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                &expected,
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &[],
                },
            )
            .await
    });
    let (early, capture) = runtime.block_on(local.run_until(async {
        producer.ready.await.unwrap();
        let early = result.is_finished();
        producer.release.send(()).unwrap();
        let capture = result.await.unwrap();
        (early, capture)
    }));
    producer.release.send(()).unwrap();
    producer.thread.join().unwrap();
    assert!(!early, "capture completed before descendant EOF");
    assert_eq!(capture.unwrap(), "earlytail");
}

#[test]
fn capture_completes_when_the_stand_in_opens_the_permit_late() {
    let producer = Producer::new();
    let scratch = scratch::Scratch::new().unwrap();
    let descendant =
        native_support::command(scratch.path(), "hold", &producer.gate(true, b"tail", b""));
    let holder = native_support::command(
        scratch.path(),
        "holder",
        &json!({"scenario": "holder", "stdout": b"early", "stderr": [], "descendant": descendant}),
    );
    let alias = holder.to_string_lossy().into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let runtime = runtime();
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                &alias,
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &[],
                },
            )
            .await
    });
    let capture = runtime.block_on(local.run_until(async {
        producer.ready.await.unwrap();
        producer.release.send(()).unwrap();
        result.await.unwrap()
    }));
    producer.release.send(()).unwrap();
    producer.thread.join().unwrap();
    assert_eq!(capture.unwrap(), "earlytail");
}

#[test]
fn capture_timeout_covers_streams_and_sends_termination() {
    let scratch = scratch::Scratch::new().unwrap();
    let producer = Producer::new();
    let marker = scratch.path().join("terminated");
    let mut record = producer.gate(false, b"", b"");
    record["terminate"] = json!({"path": marker, "cwd": false, "bytes": b"terminated"});
    let alias = native_support::command(scratch.path(), "gated", &record);
    let expected = alias.to_string_lossy().into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let runtime = runtime();
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                &expected,
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: Some(Duration::from_millis(100)),
                    env: &[],
                },
            )
            .await
    });
    let ready = producer.ready;
    runtime.block_on(local.run_until(async {
        // The deadline starts on a frozen clock; a blocking wait keeps the
        // clock from advancing until the stand-in has installed its handler.
        tokio::time::pause();
        tokio::task::spawn_blocking(move || ready.blocking_recv())
            .await
            .unwrap()
            .unwrap();
        producer.release.send(()).unwrap();
        tokio::time::advance(Duration::from_millis(101)).await;
        tokio::time::resume();
        assert_eq!(
            result.await.unwrap().unwrap_err().to_string(),
            format!("{}  timed out after 100ms", alias.to_string_lossy())
        );
    }));
    producer.release.send(()).unwrap();
    producer.thread.join().unwrap();
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "terminated");
}

#[test]
fn capture_timeout_covers_descendant_streams() {
    let runtime = runtime();
    let producer = Producer::new();
    let scratch = scratch::Scratch::new().unwrap();
    let descendant =
        native_support::command(scratch.path(), "hold", &producer.gate(false, b"tail", b""));
    let holder = native_support::command(
        scratch.path(),
        "holder",
        &json!({"scenario": "holder", "stdout": [], "stderr": [], "descendant": descendant}),
    );
    let alias = holder.to_string_lossy().into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                &alias,
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: Some(Duration::from_millis(10000)),
                    env: &[],
                },
            )
            .await
    });
    runtime.block_on(local.run_until(async {
        producer.ready.await.unwrap();
        tokio::time::pause();
        tokio::time::advance(Duration::from_millis(10001)).await;
        tokio::time::resume();
        producer.release.send(()).unwrap();
        assert_eq!(
            result.await.unwrap().unwrap_err().to_string(),
            format!("{}  timed out after 10000ms", holder.to_string_lossy())
        );
    }));
    producer.release.send(()).unwrap();
    producer.thread.join().unwrap();
}

#[test]
fn capture_spawn_error_does_not_gain_command_wrapping() {
    let scratch = scratch::Scratch::new().unwrap();
    let alias = native_support::command(
        scratch.path(),
        "present",
        &native_support::output(b"", b"", 0),
    );
    let missing = scratch
        .path()
        .join("missing")
        .to_string_lossy()
        .into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    for (command, cwd) in [
        (missing.as_str(), None),
        (alias.to_str().unwrap(), Some(missing.as_str())),
    ] {
        let result = runtime().block_on(operations.run_command_capture(
            command,
            &[],
            CommandCaptureOptions {
                cwd,
                timeout: Some(Duration::ZERO),
                env: &[],
            },
        ));
        let error = result.unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!error.to_string().contains("failed with"));
        assert!(!error.to_string().contains("timed out"));
    }
}

#[test]
fn capture_replaces_invalid_utf8() {
    assert_eq!(
        captured("partial", &native_support::output(b"\xE2", b"", 0))
            .result
            .unwrap(),
        "\u{FFFD}"
    );
    assert_eq!(
        captured("invalid", &native_support::output(b"\xFF", b"", 0))
            .result
            .unwrap(),
        "\u{FFFD}"
    );
}

#[test]
fn stopped_native_runtime_rejects_spawn_without_a_cycle() {
    use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
    use maestro_settings::{Settings, SettingsManager};
    use std::cell::RefCell;
    if crate::resolution_support::case_in_child("online", |_| vec![]).is_none() {
        return;
    }
    let local = Rc::new(tokio::task::LocalSet::new());
    let weak = Rc::downgrade(&local);
    let operations = NativePackageOperations::new(Rc::new(|| false), &local);
    let settings = Rc::new(RefCell::new(
        SettingsManager::in_memory(Settings::default()),
    ));
    let manager = Rc::new(DefaultPackageManager::new(
        PackageManagerOptions {
            cwd: "/absolute".into(),
            agent_dir: "/absolute-agent".into(),
            settings_manager: settings.clone(),
        },
        operations,
    ));
    drop(local);
    assert!(weak.upgrade().is_none());
    assert!(
        runtime()
            .block_on(manager.check_for_available_updates())
            .unwrap()
            .is_empty()
    );
    settings
        .borrow_mut()
        .set_project_packages(vec![maestro_settings::PackageSource::Source(
            "npm:x".into(),
        )]);
    assert_eq!(
        runtime()
            .block_on(manager.check_for_available_updates())
            .unwrap_err()
            .to_string(),
        "package runtime is no longer available"
    );
}

#[test]
fn captured_command_reassembles_fragmented_utf8() {
    let record = json!({"scenario": "output", "stdout": b"\xE2\x82\xAC", "stderr": [], "code": 0, "split": true});
    assert_eq!(captured("fragmented", &record).result.unwrap(), "\u{20AC}");
}
