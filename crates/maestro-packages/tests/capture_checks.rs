//! Captured native children through the public package operations.
#![cfg(test)]
#![cfg(unix)]
mod native_support;
use maestro_packages::{CommandCaptureOptions, NativePackageOperations, PackageOperations};
use std::{io, rc::Rc, time::Duration};

/// A native runtime with process and timer support.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
/// Captures an owned shell fixture through the native adapter.
fn shell(script: &str, timeout: Option<Duration>) -> io::Result<String> {
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(
        |_| false,
        Rc::new(|| panic!("capture queried stdout takeover")),
        &local,
    );
    runtime().block_on(operations.run_command_capture(
        "/bin/sh",
        &["-c".into(), script.into()],
        CommandCaptureOptions {
            cwd: None,
            timeout,
            env: &[],
        },
    ))
}
/// One descendant controlled through native FIFO producer acknowledgements.
struct Producer {
    /// Owned paths.
    scratch: native_support::Scratch,
    /// Release sent only after the producer's readiness witness.
    release: std::sync::mpsc::Sender<()>,
    /// The joined native FIFO peer.
    thread: std::thread::JoinHandle<()>,
    /// Readiness after the shell reaches its holding point.
    ready: tokio::sync::oneshot::Receiver<()>,
}
impl Producer {
    /// Creates native pipes, with a thread that acknowledges and waits for release.
    fn new() -> Self {
        use std::io::Read;
        let scratch = native_support::Scratch::new().unwrap();
        let ready = scratch.0.join("ready");
        let permit = scratch.0.join("permit");
        assert!(
            std::process::Command::new("mkfifo")
                .args([&ready, &permit])
                .status()
                .unwrap()
                .success()
        );
        let (send, receive) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let mut text = String::new();
            std::fs::File::open(ready)
                .unwrap()
                .read_to_string(&mut text)
                .unwrap();
            assert_eq!(text, "ready");
            send.send(()).unwrap();
            wait.recv().unwrap();
            std::fs::write(permit, "go\n").unwrap();
        });
        Self {
            scratch,
            release,
            thread,
            ready: receive,
        }
    }
    /// The shell's acknowledgement followed by its blocking read.
    fn gate(&self) -> String {
        format!(
            "printf ready > '{}'; read permit < '{}'",
            self.scratch.0.join("ready").display(),
            self.scratch.0.join("permit").display()
        )
    }
}

#[test]
fn captured_success_uses_stdout_only_and_exact_trim() {
    assert_eq!(
        shell(
            "printf '\\357\\273\\277  alpha  beta\\n '; printf ignored >&2",
            None
        )
        .unwrap(),
        "alpha  beta"
    );
    assert_eq!(shell("printf diagnostic >&2", None).unwrap(), "");
    assert_eq!(
        shell("printf '\\302\\205x\\302\\205'", None).unwrap(),
        "\u{85}x\u{85}"
    );
}

#[test]
fn capture_failure_keeps_stream_precedence_and_signal() {
    for (script, status, payload) in [
        (
            "printf output; printf ' diagnostic\\n' >&2; exit 7",
            "code 7",
            " diagnostic\n",
        ),
        ("printf ' output\\n'; exit 8", "code 8", " output\n"),
        ("printf output; printf ' ' >&2; exit 9", "code 9", " "),
        ("kill -TERM $$", "signal SIGTERM", ""),
    ] {
        assert_eq!(
            shell(script, None).unwrap_err().to_string(),
            format!("/bin/sh -c {script} failed with {status}: {payload}")
        );
    }
    #[cfg(target_os = "linux")]
    assert_eq!(
        shell("kill -34 $$", None).unwrap_err().to_string(),
        "/bin/sh -c kill -34 $$ failed with signal unknown: "
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    assert_eq!(
        runtime()
            .block_on(operations.run_command_capture(
                "/bin/false",
                &[],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &[]
                }
            ))
            .unwrap_err()
            .to_string(),
        "/bin/false  failed with code 1: "
    );
}

#[test]
fn capture_inherits_environment_and_applies_scoped_overrides() {
    let scratch = native_support::Scratch::new().unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    let args=["-c", "test -n \"$PATH\"; test -z \"$(cat)\"; printf '%s|%s|%s' \"$MAESTRO_CAPTURE_VALUE\" \"$PWD\" \"$1\"", "fixture", "a; b  c"].map(str::to_owned);
    let cwd = scratch.0.to_string_lossy();
    let output = runtime()
        .block_on(operations.run_command_capture(
            "/bin/sh",
            &args,
            CommandCaptureOptions {
                cwd: Some(&cwd),
                timeout: None,
                env: &[("MAESTRO_CAPTURE_VALUE", "chosen")],
            },
        ))
        .unwrap();
    assert_eq!(output, format!("chosen|{cwd}|a; b  c"));
}

#[test]
fn capture_uses_shell_query_without_stdout_takeover() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static QUERIES: AtomicUsize = AtomicUsize::new(0);
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(
        |command| {
            assert_eq!(command, "printf");
            QUERIES.fetch_add(1, Ordering::SeqCst);
            true
        },
        Rc::new(|| panic!("capture queried takeover")),
        &local,
    );
    let args = ["'%s'", "'literal text'"].map(str::to_owned);
    let runtime = runtime();
    for _ in 0..2 {
        assert_eq!(
            runtime
                .block_on(operations.run_command_capture(
                    "printf",
                    &args,
                    CommandCaptureOptions {
                        cwd: None,
                        timeout: None,
                        env: &[]
                    }
                ))
                .unwrap(),
            "literal text"
        );
    }
    assert_eq!(QUERIES.load(Ordering::SeqCst), 2);
}

#[test]
fn capture_waits_for_exit_and_both_pipe_eofs() {
    let producer = Producer::new();
    let script = format!(
        "parent=$$; (while kill -0 \"$parent\" 2>/dev/null; do :; done; {}; printf tail; printf diagnostic >&2) & printf early; exit 0",
        producer.gate()
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    let runtime = runtime();
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                "/bin/sh",
                &["-c".into(), script],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: None,
                    env: &[],
                },
            )
            .await
    });
    let (early, captured) = runtime.block_on(local.run_until(async {
        tokio::time::timeout(Duration::from_secs(5), producer.ready)
            .await
            .unwrap()
            .unwrap();
        let early = result.is_finished();
        producer.release.send(()).unwrap();
        let captured = tokio::time::timeout(Duration::from_secs(5), result)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        (early, captured)
    }));
    producer.thread.join().unwrap();
    assert!(!early, "capture completed before descendant EOF");
    assert_eq!(captured, "earlytail");
}

#[test]
fn capture_timeout_covers_streams_and_sends_termination() {
    let scratch = native_support::Scratch::new().unwrap();
    let marker = scratch.0.join("terminated");
    native_support::write(marker.to_str().unwrap(), "waiting");
    let producer = Producer::new();
    let script = format!(
        "trap 'printf terminated > {}; exit 0' TERM; {}; while :; do :; done",
        marker.display(),
        producer.gate()
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    let runtime = runtime();
    let expected = format!("/bin/sh -c {script} timed out after 100ms");
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                "/bin/sh",
                &["-c".into(), script],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: Some(Duration::from_millis(100)),
                    env: &[],
                },
            )
            .await
    });
    runtime.block_on(local.run_until(async {
        tokio::time::timeout(Duration::from_secs(5), producer.ready)
            .await
            .unwrap()
            .unwrap();
        tokio::time::pause();
        tokio::time::advance(Duration::from_millis(101)).await;
        tokio::time::resume();
        producer.release.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), result)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .to_string(),
            expected
        );
    }));
    producer.thread.join().unwrap();
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "terminated");
}

#[test]
fn capture_timeout_covers_descendant_streams() {
    let runtime = runtime();
    let producer = Producer::new();
    let script = format!(
        "parent=$$; (while kill -0 \"$parent\" 2>/dev/null; do :; done; {}; printf tail) & exit 0",
        producer.gate()
    );
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    let expected = format!("/bin/sh -c {script} timed out after 10000ms");
    let result = local.spawn_local(async move {
        operations
            .run_command_capture(
                "/bin/sh",
                &["-c".into(), script],
                CommandCaptureOptions {
                    cwd: None,
                    timeout: Some(Duration::from_millis(10000)),
                    env: &[],
                },
            )
            .await
    });
    runtime.block_on(local.run_until(async {
        tokio::time::timeout(Duration::from_secs(5), producer.ready)
            .await
            .unwrap()
            .unwrap();
        tokio::time::pause();
        tokio::time::advance(Duration::from_millis(10001)).await;
        tokio::time::resume();
        producer.release.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), result)
                .await
                .unwrap()
                .unwrap()
                .unwrap_err()
                .to_string(),
            expected
        );
    }));
    producer.thread.join().unwrap();
}

#[test]
fn capture_spawn_error_does_not_gain_command_wrapping() {
    let scratch = native_support::Scratch::new().unwrap();
    let missing = scratch.0.join("missing").to_string_lossy().into_owned();
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
    for (command, cwd) in [
        (missing.as_str(), None),
        ("/bin/sh", Some(missing.as_str())),
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
    assert_eq!(shell("printf '\\342'", None).unwrap(), "�");
    assert_eq!(shell("printf '\\377'", None).unwrap(), "�");
}

#[test]
fn stopped_native_runtime_rejects_spawn_without_a_cycle() {
    use maestro_packages::{DefaultPackageManager, PackageManagerOptions};
    use maestro_settings::{Settings, SettingsManager};
    use std::cell::RefCell;
    let local = Rc::new(tokio::task::LocalSet::new());
    let weak = Rc::downgrade(&local);
    let operations = NativePackageOperations::new(|_| false, Rc::new(|| false), &local);
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
