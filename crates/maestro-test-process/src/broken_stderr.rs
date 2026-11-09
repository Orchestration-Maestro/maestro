//! A child whose standard error has lost its reader.

use std::io::Read;
use std::process::{ExitStatus, Stdio};

use crate::{TestProcess, TestProcessError};

/// Runs `process` as a child whose standard error is a pipe nobody reads, and returns its
/// exit status.
///
/// The child prints its own output as it happens, so a diagnostic written with a print macro
/// reaches the broken pipe instead of the harness capture. It stays idle in
/// [`wait_for_broken_stderr`] until this process has dropped the reading end, so no write
/// can win the race against the pipe's closing. The calling test starts with [`crate::run_alone`],
/// so no sibling test forks while the pipe's reading end is open.
///
/// # Errors
///
/// Fails when the child cannot be selected, started or waited for.
pub fn run_with_broken_stderr(process: TestProcess) -> Result<ExitStatus, TestProcessError> {
    let mut child = process
        .nocapture()
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    drop(child.stderr.take());
    child.wait()
}

/// Blocks a child launched by [`run_with_broken_stderr`] until its standard error has no
/// reader, which the launcher signals by closing the child's standard input.
pub fn wait_for_broken_stderr() {
    let mut released = Vec::new();
    // A failed read still means the launcher is gone, so the pipe is closed.
    let _ = std::io::stdin().read_to_end(&mut released);
}
