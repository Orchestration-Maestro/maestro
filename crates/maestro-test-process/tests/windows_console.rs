//! Fresh-console startup, caller-selected handles and completion proof.
#![cfg(windows)]

use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::process::Command;

use maestro_test_process::{TestProcess, on_console, test_name};

/// Select the console probe's failure mode without changing the test process environment.
const BEHAVIOR: &str = "MAESTRO_TEST_CONSOLE_BEHAVIOR";

/// Attach the scenario's input and output to the host's console buffers.
fn console_handles(command: &mut Command) -> &mut Command {
    let console = |name| {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .unwrap()
    };
    command.stdin(console("CONIN$")).stdout(console("CONOUT$"))
}

#[test]
fn console_runs_the_body_with_caller_selected_handles() {
    on_console(
        "console_runs_the_body_with_caller_selected_handles",
        console_handles,
        || {
            assert!(std::io::stdin().is_terminal());
            assert!(std::io::stdout().is_terminal());
        },
    )
    .unwrap();
}

#[test]
fn console_rejects_success_without_body_completion() {
    assert_failed_probe("exit", "did not finish");
}

#[test]
fn console_reports_a_scenario_panic() {
    assert_failed_probe("panic", "console scenario panic");
}

/// Run the controlled probe and require a failed test with the selected diagnostic.
fn assert_failed_probe(behavior: &str, diagnostic: &str) {
    let output = TestProcess::named(test_name(console_failure_probe).unwrap())
        .unwrap()
        .env(BEHAVIOR, behavior)
        .nocapture()
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(diagnostic), "{stderr}");
}

#[test]
fn console_failure_probe() {
    let Ok(behavior) = std::env::var(BEHAVIOR) else {
        return;
    };
    on_console("console_failure_probe", console_handles, || {
        if behavior == "exit" {
            std::process::exit(0);
        }
        panic!("console scenario panic");
    })
    .unwrap();
}
