//! Native signal names for children that end without an exit code.
#![cfg(test)]
#![cfg(unix)]

use crate::child_programs;
use maestro_process::exit_signal_name;
use std::process::ExitStatus;

/// The status of a controlled child that exits with `CHILD_EXIT` or raises `CHILD_SIGNAL`.
fn status(variables: &[(&str, &str)]) -> ExitStatus {
    child_programs::command(child_programs::output_writer, variables)
        .status()
        .expect("the child starts")
}

#[test]
fn native_signal_names_preserve_capture_results() {
    assert_eq!(exit_signal_name(status(&[("CHILD_EXIT", "0")])), None);
    assert_eq!(exit_signal_name(status(&[("CHILD_EXIT", "23")])), None);
    assert_eq!(
        exit_signal_name(status(&[("CHILD_SIGNAL", "15")])),
        Some("SIGTERM")
    );
    assert_eq!(
        exit_signal_name(status(&[("CHILD_SIGNAL", "6")])),
        Some("SIGABRT")
    );
    assert_eq!(
        exit_signal_name(status(&[("CHILD_SIGNAL", "2")])),
        Some("SIGINT")
    );
    #[cfg(target_os = "linux")]
    assert_eq!(exit_signal_name(status(&[("CHILD_SIGNAL", "34")])), None);
}
