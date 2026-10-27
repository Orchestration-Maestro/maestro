//! Explicit command execution: a literal program and arguments in a given directory.
//!
//! Lifecycle, pipe collection and termination belong to
//! [`maestro_process::child_process`]; this module selects the exit grace and the
//! escalation delay and maps the outcome to an [`ExecResult`].
use maestro_models::Cancellation;
use maestro_process::child_process::{
    CapturedChild, EXIT_STDIO_GRACE_MS, Termination, WaitOptions, wait_for_child_process,
};
use std::{cell::Cell, io, process::Output, time::Duration};

/// How long pipes may stay open after the command exits.
const EXIT_GRACE: Duration = Duration::from_millis(EXIT_STDIO_GRACE_MS);
/// How long a terminated command may keep running before it is killed.
const KILL_AFTER: Duration = Duration::from_secs(5);

/// Cancellation and deadline for one execution.
#[derive(Default)]
pub struct ExecOptions<'a> {
    /// Terminates the command when aborted.
    pub signal: Option<&'a Cancellation>,
    /// Terminates the command after this long; `None` and zero set no deadline.
    pub timeout: Option<Duration>,
}

/// What one execution produced.
pub struct ExecResult {
    /// The stdout bytes collected by the wait, decoded lossily.
    pub stdout: String,
    /// The stderr bytes collected by the wait, decoded lossily.
    pub stderr: String,
    /// The exit code; 0 when a signal ended the command, 1 when it did not start or
    /// waiting failed.
    pub code: i32,
    /// Whether the abort or the timeout requested termination.
    pub killed: bool,
}

/// Runs `command` with `args` in `cwd`, without a shell and with stdin closed.
///
/// An empty `cwd` leaves the working directory inherited. A command that cannot
/// start (missing program or directory, permission denied) yields code 1 with empty
/// streams. Which bytes a finished or failed wait collects
/// is decided by [`wait_for_child_process`].
///
/// # Errors
/// Returns `InvalidInput` for an empty `command`, before the signal is observed, and
/// the native error for an operand the operating system rejects as invalid.
pub async fn exec_command(
    command: &str,
    args: &[String],
    cwd: &str,
    options: ExecOptions<'_>,
) -> io::Result<ExecResult> {
    if command.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty command"));
    }
    let mut builder = maestro_process::command(command);
    builder.args(args);
    if !cwd.is_empty() {
        builder.current_dir(cwd);
    }
    let mut child = match CapturedChild::spawn(&mut builder) {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => return Err(error),
        Err(_) => {
            return Ok(ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                code: 1,
                killed: options.signal.is_some_and(Cancellation::is_aborted),
            });
        }
    };
    let killed = Cell::new(false);
    let stop = async {
        let aborted = async {
            match options.signal {
                Some(signal) => signal.cancelled().await,
                None => std::future::pending().await,
            }
        };
        let expired = async {
            match options.timeout.filter(|timeout| !timeout.is_zero()) {
                Some(timeout) => tokio::time::sleep(timeout).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! { () = aborted => {}, () = expired => {} }
        killed.set(true);
        Termination::TerminateThenKill { after: KILL_AFTER }
    };
    let wait = WaitOptions {
        post_exit_grace: Some(EXIT_GRACE),
        ..WaitOptions::default()
    };
    let waited = wait_for_child_process(&mut child, wait, stop, None).await;
    Ok(project(waited, child.take_output(), killed.get()))
}

/// Maps a finished wait to a result, using `partial` when the wait failed.
fn project(waited: io::Result<Output>, partial: (Vec<u8>, Vec<u8>), killed: bool) -> ExecResult {
    let (code, (stdout, stderr)) = match waited {
        Ok(output) => (
            output.status.code().unwrap_or(0),
            (output.stdout, output.stderr),
        ),
        Err(_) => (1, partial),
    };
    ExecResult {
        stdout: maestro_text::lossy_string(stdout),
        stderr: maestro_text::lossy_string(stderr),
        code,
        killed,
    }
}

#[cfg(test)]
mod tests;
