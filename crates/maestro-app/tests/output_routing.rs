//! Managed stream routing observed in isolated native processes.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::package_fixtures as fixtures;
use maestro_app::output_guard::{
    flush_raw_stdout, is_stdout_taken_over, restore_stdout, take_over_stdout, write_raw_stdout,
    write_stdout,
};
use serde::Deserialize;
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard, PoisonError},
};
/// Caller text, including empty and control-character inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    text: String,
}
/// Exact observed streams and state transitions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Routed {
    stdout: String,
    stderr: String,
    states: Vec<bool>,
}
/// Re-enter the named test with piped streams, bypassing harness teardown on exit.
fn child(name: &str, mode: &str) -> Command {
    let name = crate::child_process::selector(name).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &name,
            "--nocapture",
            "--test-threads=1",
            "--quiet",
        ])
        .env("MAESTRO_OUTPUT_PROBE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}
/// Orders this file's child launches; a fork elsewhere in the test program can still hold a pipe end until it execs.
static LAUNCH: Mutex<()> = Mutex::new(());
/// Acquire the launch lock; it stays held until the returned guard is dropped.
fn launch_gate() -> MutexGuard<'static, ()> {
    LAUNCH.lock().unwrap_or_else(PoisonError::into_inner)
}
/// The harness prologue is separate from product bytes.
fn product(bytes: &[u8]) -> &[u8] {
    let prefix = b"\nrunning 1 test\n";
    assert!(bytes.starts_with(prefix));
    &bytes[prefix.len()..]
}
/// Only managed ordinary output changes streams during takeover.
#[test]
fn takeover_routes_diagnostics_without_polluting_raw_stdout() {
    let cases = fixtures::cases::<Text, Routed>("route-");
    if let Ok(id) = std::env::var("MAESTRO_OUTPUT_PROBE") {
        let case = cases.iter().find(|c| c.id == id).unwrap();
        let mut states = vec![is_stdout_taken_over()];
        restore_stdout();
        states.push(is_stdout_taken_over());
        write_stdout(&format!("before:{}", case.input.text)).unwrap();
        write_raw_stdout(&format!("raw-before:{}", case.input.text)).unwrap();
        take_over_stdout();
        take_over_stdout();
        states.push(is_stdout_taken_over());
        write_stdout(&format!("during:{}", case.input.text)).unwrap();
        write_raw_stdout(&format!("raw-during:{}", case.input.text)).unwrap();
        restore_stdout();
        restore_stdout();
        states.push(is_stdout_taken_over());
        write_stdout(&format!("after:{}", case.input.text)).unwrap();
        write_raw_stdout(&format!("raw-after:{}", case.input.text)).unwrap();
        flush_raw_stdout().unwrap();
        assert_eq!(states, case.expected.states);
        std::process::exit(0);
    }
    let _gate = launch_gate();
    for case in cases {
        let output = child(
            "takeover_routes_diagnostics_without_polluting_raw_stdout",
            &case.id,
        )
        .output()
        .unwrap();
        assert!(output.status.success());
        assert_eq!(product(&output.stdout), case.expected.stdout.as_bytes());
        assert_eq!(output.stderr, case.expected.stderr.as_bytes());
    }
}
/// Recorded stdout ownership and flush outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlushInput {
    taken: bool,
    failed: bool,
}
/// The source's successful or failed flush branch.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlushExpected {
    branch: String,
}
/// The operation performed after the parent closes or retains the selected pipe.
#[derive(Clone, Copy, PartialEq)]
enum PipeOperation {
    Flush,
    Ordinary,
    Raw,
}
/// One isolated pipe transaction.
#[derive(Clone, Copy)]
struct PipeCase {
    operation: PipeOperation,
    taken: bool,
    failed: bool,
}
/// Read one exact protocol acknowledgement.
fn line(reader: &mut impl BufRead, expected: &str) {
    let mut value = String::new();
    reader.read_line(&mut value).unwrap();
    assert_eq!(value, expected);
}
/// Drive the real buffered stream operation only after the parent's acknowledgement.
fn pipe_probe(case: PipeCase) -> ! {
    if case.taken {
        take_over_stdout();
    }
    let error_stream = case.operation == PipeOperation::Ordinary && case.taken;
    if case.operation == PipeOperation::Flush {
        flush_raw_stdout().unwrap();
        flush_raw_stdout().unwrap();
        write_raw_stdout("pending").unwrap();
    }
    if error_stream {
        write_raw_stdout("READY\n").unwrap();
    } else {
        io::stderr().write_all(b"READY\n").unwrap();
    }
    let mut acknowledgement = String::new();
    io::stdin().read_line(&mut acknowledgement).unwrap();
    assert_eq!(acknowledgement, "continue\n");
    let result = match case.operation {
        PipeOperation::Flush => flush_raw_stdout(),
        PipeOperation::Ordinary => write_stdout("ordinary\n"),
        PipeOperation::Raw => write_raw_stdout("raw\n"),
    };
    match result {
        Ok(()) => assert!(!case.failed),
        Err(error) => {
            assert!(case.failed);
            assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        }
    }
    let result = if case.failed { "error\n" } else { "ok\n" };
    if error_stream {
        write_raw_stdout(result).unwrap();
    } else {
        io::stderr().write_all(result.as_bytes()).unwrap();
    }
    if case.operation == PipeOperation::Flush && !case.failed {
        acknowledgement.clear();
        io::stdin().read_line(&mut acknowledgement).unwrap();
        assert_eq!(acknowledgement, "observed\n");
    }
    std::process::exit(0);
}
/// Complete one pipe handshake, joining the child before releasing observations.
fn pipe_case(name: &str, case: PipeCase) {
    let mode = format!(
        "{}{}:{}",
        if case.operation == PipeOperation::Raw {
            "raw:"
        } else {
            ""
        },
        case.taken,
        case.failed
    );
    if std::env::var("MAESTRO_OUTPUT_PROBE").is_ok() {
        pipe_probe(case);
    }
    let _gate = launch_gate();
    let mut process = child(name, &mode).spawn().unwrap();
    let mut stdout = BufReader::new(process.stdout.take().unwrap());
    line(&mut stdout, "\n");
    line(&mut stdout, "running 1 test\n");
    let mut stderr = BufReader::new(process.stderr.take().unwrap());
    let error_stream = case.operation == PipeOperation::Ordinary && case.taken;
    if error_stream {
        line(&mut stdout, "READY\n");
        if case.failed {
            drop(stderr);
        } else {
            panic!("ordinary error-stream neighbour not selected");
        }
        process
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"continue\n")
            .unwrap();
        line(&mut stdout, "error\n");
    } else {
        line(&mut stderr, "READY\n");
        if case.failed {
            drop(stdout);
        } else if case.operation == PipeOperation::Flush {
            observe_flushed(&mut process, &mut stdout, &mut stderr);
            return;
        }
        process
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"continue\n")
            .unwrap();
        line(&mut stderr, if case.failed { "error\n" } else { "ok\n" });
    }
    assert!(process.wait().unwrap().success());
}
/// Unterminated raw bytes are visible after flush, before the child exits.
#[test]
fn raw_stdout_preserves_unterminated_text_and_flushes() {
    if let Ok(mode) = std::env::var("MAESTRO_OUTPUT_PROBE") {
        pipe_probe(PipeCase {
            operation: PipeOperation::Flush,
            taken: mode.starts_with("true"),
            failed: false,
        });
    }
    for taken in [false, true] {
        pipe_case(
            "raw_stdout_preserves_unterminated_text_and_flushes",
            PipeCase {
                operation: PipeOperation::Flush,
                taken,
                failed: false,
            },
        );
    }
}
/// Actual pipe closure propagates the native flush failure in both ownership states.
#[test]
fn raw_stdout_reports_flush_failure() {
    let cases = fixtures::cases::<FlushInput, FlushExpected>("flush-");
    if let Ok(mode) = std::env::var("MAESTRO_OUTPUT_PROBE") {
        pipe_probe(PipeCase {
            operation: PipeOperation::Flush,
            taken: mode.starts_with("true"),
            failed: mode.ends_with("true"),
        });
    }
    for case in cases {
        assert_eq!(
            case.id,
            format!("flush-{}-{}", case.input.taken, case.input.failed)
        );
        assert_eq!(
            case.expected.branch,
            if case.input.failed { "error" } else { "ok" }
        );
        pipe_case(
            "raw_stdout_reports_flush_failure",
            PipeCase {
                operation: PipeOperation::Flush,
                taken: case.input.taken,
                failed: case.input.failed,
            },
        );
    }
}
/// A healthy sibling pipe cannot hide a selected stream's write error.
#[test]
fn ordinary_stdout_reports_write_failure() {
    if let Ok(mode) = std::env::var("MAESTRO_OUTPUT_PROBE") {
        let operation = if mode.starts_with("raw") {
            PipeOperation::Raw
        } else {
            PipeOperation::Ordinary
        };
        pipe_probe(PipeCase {
            operation,
            taken: mode
                .strip_prefix("raw:")
                .unwrap_or(&mode)
                .starts_with("true"),
            failed: true,
        });
    }
    for taken in [false, true] {
        pipe_case(
            "ordinary_stdout_reports_write_failure",
            PipeCase {
                operation: PipeOperation::Ordinary,
                taken,
                failed: true,
            },
        );
    }
    pipe_case(
        "ordinary_stdout_reports_write_failure",
        PipeCase {
            operation: PipeOperation::Raw,
            taken: true,
            failed: true,
        },
    );
}
/// Input inherited by the ordinary subprocesses.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stdin {
    stdin: Vec<String>,
}
/// Exact subprocess streams.
#[cfg(unix)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Streams {
    stdout: String,
    stderr: String,
}
/// One native adapter reads the shared query again for every child launch.
#[cfg(unix)]
#[test]
fn package_children_observe_takeover_changes_after_construction() {
    let cases = fixtures::cases::<Stdin, Streams>("live-package-");
    assert_eq!(cases.len(), 1);
    let case = &cases[0];
    assert_eq!(case.id, "live-package-stdio");
    if std::env::var("MAESTRO_OUTPUT_PROBE").is_ok() {
        package_probe();
        std::process::exit(0);
    }
    let _gate = launch_gate();
    let mut process = child(
        "package_children_observe_takeover_changes_after_construction",
        &case.id,
    )
    .spawn()
    .unwrap();
    for text in &case.input.stdin {
        writeln!(process.stdin.as_mut().unwrap(), "{text}").unwrap();
    }
    drop(process.stdin.take());
    let output = process.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(product(&output.stdout), case.expected.stdout.as_bytes());
    assert_eq!(output.stderr, case.expected.stderr.as_bytes());
}

/// Observe unterminated bytes while the child is waiting for acknowledgement.
fn observe_flushed(
    process: &mut std::process::Child,
    stdout: &mut impl Read,
    stderr: &mut impl BufRead,
) {
    // A second signal separates completed flush from the still-live child exit.
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"continue\n")
        .unwrap();
    line(stderr, "ok\n");
    let mut bytes = [0; 7];
    stdout.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"pending");
    process
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"observed\n")
        .unwrap();
    assert!(process.wait().unwrap().success());
}

/// Run all three launches on the same retained native adapter.
#[cfg(unix)]
fn package_probe() {
    use maestro_packages::{NativePackageOperations, PackageOperations};
    use std::rc::Rc;
    let local = Rc::new(tokio::task::LocalSet::new());
    let operations = NativePackageOperations::new(|_| false, Rc::new(is_stdout_taken_over), &local);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(local.run_until(async {
            for label in ["A", "B", "C"] {
                if label == "B" { take_over_stdout(); }
                if label == "C" { restore_stdout(); }
                let args = ["-c", "printf '%s\\n' \"$1\"; printf '%s-err\\n' \"$1\" >&2; value=; IFS= read -r value || :; printf 'stdin:%s\\n' \"$value\"", "literal shell name", label].map(str::to_owned);
                assert_eq!(operations.run_command("/bin/sh", &args, None).await.unwrap(), Some(0));
            }
        }));
    runtime.block_on(Rc::try_unwrap(local).ok().unwrap());
}
