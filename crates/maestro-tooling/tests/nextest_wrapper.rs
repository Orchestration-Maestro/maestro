//! Fixed-port admission through the development command's child boundary.

use std::fs::OpenOptions;
use std::process::Command;

use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;

/// Variable naming the lock file the wrapper holds while its child runs.
const LOCK: &str = "MAESTRO_FIXED_PORT_LOCK";

#[test]
fn fixed_port_wrapper_holds_lock_while_child_runs() {
    if maestro_test_process::selection().is_child() {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(std::env::var_os(LOCK).unwrap())
            .unwrap();
        assert!(matches!(
            file.try_lock(),
            Err(std::fs::TryLockError::WouldBlock)
        ));
        std::process::exit(37);
    }
    let scratch = Scratch::new().unwrap();
    let path = scratch.path().join("fixed-port.lock");
    std::fs::write(&path, b"retained").unwrap();
    let output =
        TestProcess::named("nextest_wrapper::fixed_port_wrapper_holds_lock_while_child_runs")
            .unwrap()
            .nocapture()
            .output_with(|arguments| {
                Command::new(env!("CARGO_BIN_EXE_development"))
                    .args(["nextest-wrapper", "fixed-port"])
                    .arg(std::env::current_exe()?)
                    .args(arguments)
                    .env(maestro_test_process::CASE, "wrapped")
                    .env(LOCK, &path)
                    .output()
            })
            .unwrap();
    assert_eq!(output.status.code(), Some(37), "{output:?}");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    file.try_lock().unwrap();
    // Windows locks also exclude reads through another handle.
    drop(file);
    assert_eq!(std::fs::read(path).unwrap(), b"retained");
}

/// A child that ends by a signal reaches the caller as a failure that names it.
///
/// The signalled status carries no exit code, so the command reports the native status text
/// instead of only a bare failure.
#[cfg(unix)]
#[test]
fn signalled_child_failure_names_its_signal() {
    if maestro_test_process::selection().is_child() {
        std::process::abort();
    }
    let output = TestProcess::named("nextest_wrapper::signalled_child_failure_names_its_signal")
        .unwrap()
        .output_with(|arguments| {
            Command::new(env!("CARGO_BIN_EXE_development"))
                .args(["nextest-wrapper", "fixed-port"])
                .arg(std::env::current_exe()?)
                .args(arguments)
                .env(maestro_test_process::CASE, "signalled")
                .env_remove(LOCK)
                .output()
        })
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("child ended by signal: 6 (SIGABRT)"),
        "{stderr}"
    );
}

#[test]
fn fixed_port_wrapper_runs_without_the_lock_variable() {
    if maestro_test_process::selection().is_child() {
        return;
    }
    let output =
        TestProcess::named("nextest_wrapper::fixed_port_wrapper_runs_without_the_lock_variable")
            .unwrap()
            .nocapture()
            .output_with(|arguments| {
                Command::new(env!("CARGO_BIN_EXE_development"))
                    .args(["nextest-wrapper", "fixed-port"])
                    .arg(std::env::current_exe()?)
                    .args(arguments)
                    .env(maestro_test_process::CASE, "wrapped")
                    .env_remove(LOCK)
                    .output()
            })
            .unwrap();
    assert!(output.status.success(), "{output:?}");
}
