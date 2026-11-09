//! Diagnostics written to a standard error nobody reads.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_agent::diagnostics::{write_stderr, write_stderr_line};
use maestro_test_process::{
    TestProcess, run_with_broken_stderr, selection, wait_for_broken_stderr,
};

/// Both writers return to their caller when the pipe behind standard error is closed.
#[test]
fn diagnostics_return_when_standard_error_is_closed() {
    const TEST: &str = "diagnostics_return_when_standard_error_is_closed";
    if !selection().is_child() {
        let status =
            run_with_broken_stderr(TestProcess::current(TEST).unwrap().env_clear().case("*"))
                .unwrap();
        assert!(status.success(), "the writer failed its caller: {status}");
        return;
    }
    wait_for_broken_stderr();
    write_stderr("without a newline");
    write_stderr_line("with a newline");
}
