//! Native signal names for children that end without an exit code.
#![cfg(test)]
#![cfg(unix)]

use maestro_process::exit_signal_name;
use std::process::Command;

/// The status of `/bin/sh` running `script`.
fn status(script: &str) -> std::process::ExitStatus {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(script)
        .status()
        .expect("the shell starts")
}

#[test]
fn native_signal_names_preserve_capture_results() {
    assert_eq!(exit_signal_name(status("exit 0")), None);
    assert_eq!(exit_signal_name(status("exit 23")), None);
    assert_eq!(exit_signal_name(status("kill -TERM $$")), Some("SIGTERM"));
    assert_eq!(exit_signal_name(status("kill -ABRT $$")), Some("SIGABRT"));
    assert_eq!(exit_signal_name(status("kill -INT $$")), Some("SIGINT"));
    #[cfg(target_os = "linux")]
    assert_eq!(exit_signal_name(status("kill -34 $$")), None);
}
