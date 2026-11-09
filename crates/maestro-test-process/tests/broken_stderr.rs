//! A child launched with a broken standard error finds the pipe closed before it writes.
#![cfg(test)]

use std::io::Write;

use maestro_test_process::{
    TestProcess, run_alone, run_with_broken_stderr, wait_for_broken_stderr,
};

/// Child body: once released, writing to standard error fails.
#[test]
#[ignore = "runs only inside a controlled child"]
fn stderr_write_fails_child() {
    wait_for_broken_stderr();
    let refused = std::io::stderr().write_all(b"diagnostic\n").is_err();
    std::process::exit(if refused { 0 } else { 3 });
}

#[test]
fn child_finds_standard_error_closed_when_released() {
    if run_alone("child_finds_standard_error_closed_when_released").unwrap() {
        return;
    }
    let status =
        run_with_broken_stderr(TestProcess::child(stderr_write_fails_child).unwrap()).unwrap();
    assert!(status.success(), "the child's write succeeded: {status}");
}
