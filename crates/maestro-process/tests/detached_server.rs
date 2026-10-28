//! The detached-daemon observation follows the native session of the running process.
#![cfg(unix)]
#![cfg(test)]
use maestro_process::child_process::{CapturedChild, WaitOptions, wait_for_child_process};
use maestro_process::foreground::current_process_is_detached_server_daemon;
use maestro_test_process::TestProcess;
use std::process::Output;

/// Marks the child run that reports its own observations.
const PROBE: &str = "MAESTRO_DETACHED_PROBE";

#[test]
fn detached_capability_matches_the_native_session() {
    if std::env::var_os(PROBE).is_some() {
        let leads = rustix::process::getsid(None).unwrap() == rustix::process::getpid();
        println!(
            "detached={} leader={leads}",
            current_process_is_detached_server_daemon()
        );
        return;
    }
    for detached in [false, true] {
        let output = TestProcess::current("detached_capability_matches_the_native_session")
            .unwrap()
            .nocapture()
            .output_with(|harness| probe(harness, detached))
            .unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        let expected = format!("detached={detached} leader={detached}");
        assert!(stdout.lines().any(|line| line == expected), "{stdout}");
    }
}

/// Runs the probe child in a new native session, or in this process's session.
fn probe(harness: &[String], detached: bool) -> std::io::Result<Output> {
    let mut command = tokio::process::Command::new(std::env::current_exe()?);
    command.args(harness).env(PROBE, "1");
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let mut child = if detached {
                CapturedChild::spawn_detached(command)
            } else {
                CapturedChild::spawn(&mut command)
            }?;
            wait_for_child_process(
                &mut child,
                WaitOptions::default(),
                std::future::pending(),
                None,
            )
            .await
        })
}
