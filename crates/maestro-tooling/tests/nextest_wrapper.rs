//! Fixed-port admission through the development command's child boundary.

use std::fs::OpenOptions;
use std::process::Command;

use maestro_test_directory::Scratch;
use maestro_test_process::TestProcess;

#[test]
fn fixed_port_wrapper_holds_lock_while_child_runs() {
    if let Some(path) = std::env::var_os("MAESTRO_WRAPPER_CHILD") {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
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
                    .env("MAESTRO_WRAPPER_CHILD", &path)
                    .env("MAESTRO_FIXED_PORT_LOCK", &path)
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

#[test]
fn fixed_port_wrapper_runs_without_the_lock_variable() {
    if std::env::var_os("MAESTRO_WRAPPER_CHILD").is_some() {
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
                    .env("MAESTRO_WRAPPER_CHILD", "1")
                    .env_remove("MAESTRO_FIXED_PORT_LOCK")
                    .output()
            })
            .unwrap();
    assert!(output.status.success(), "{output:?}");
}
