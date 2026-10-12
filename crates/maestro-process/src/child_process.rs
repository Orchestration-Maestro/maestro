//! Captured child processes: spawn, raw pipe collection, exit and EOF observation,
//! termination and reaping.
//!
//! Callers keep cancellation, deadlines, output decoding and error text.
use std::future::Future;
use std::io;
use std::process::{Output, Stdio};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};

/// A spawned child whose stdout and stderr are captured and whose stdin is closed.
pub struct CapturedChild {
    /// The native child, which this value reaps.
    child: Child,
    /// Stdout, until a wait takes it.
    stdout: Option<ChildStdout>,
    /// Stderr, until a wait takes it.
    stderr: Option<ChildStderr>,
}

/// Whether stderr bytes are returned or consumed and dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StderrMode {
    /// Return every stderr byte.
    Capture,
    /// Read stderr to its end and return nothing.
    Discard,
}

/// The native action a caller requests for a running child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// Ordinary platform termination, with no wait and no escalation.
    Terminate,
    /// Forced kill of the direct child, awaited until it is reaped.
    Kill,
}

impl CapturedChild {
    /// Spawns `command` with ignored stdin and captured stdout and stderr.
    ///
    /// # Errors
    /// Returns the native spawn error unchanged.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn()?;
        Ok(Self {
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child,
        })
    }

    /// Applies `termination` to the child if it has not already exited.
    ///
    /// The child's current state decides whether a signal is sent, so a reaped
    /// child is never signalled. [`Termination::Terminate`] ignores native
    /// failures; [`Termination::Kill`] propagates them and returns once the
    /// direct child is reaped. Descendants and process groups are untouched.
    ///
    /// # Errors
    /// Returns the native status or kill error of a [`Termination::Kill`].
    pub async fn terminate(&mut self, termination: Termination) -> io::Result<()> {
        let exited = self.child.try_wait();
        match termination {
            Termination::Kill => {
                if exited?.is_none() {
                    self.child.kill().await?;
                }
            }
            Termination::Terminate => {
                if !matches!(exited, Ok(Some(_))) {
                    request_termination(&mut self.child);
                }
            }
        }
        Ok(())
    }
}

/// Sends the ordinary termination signal, or kills where none exists.
fn request_termination(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
    #[cfg(not(unix))]
    let _ = child.start_kill();
}

/// Reads a pipe to its end, keeping its bytes only when `mode` captures them.
///
/// A pipe already taken by an earlier wait yields no bytes.
async fn collect(pipe: Option<impl AsyncRead + Unpin>, mode: StderrMode) -> io::Result<Vec<u8>> {
    let Some(mut pipe) = pipe else {
        return Ok(Vec::new());
    };
    let mut bytes = Vec::new();
    match mode {
        StderrMode::Capture => {
            pipe.read_to_end(&mut bytes).await?;
        }
        StderrMode::Discard => {
            tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await?;
        }
    }
    Ok(bytes)
}

/// Waits for the exit and both pipe EOFs, honouring one termination request.
///
/// `termination` is polled until it yields once. [`Termination::Terminate`]
/// keeps collecting until exit and both EOFs; [`Termination::Kill`] stops the
/// readers and returns once the direct child is reaped, with whatever bytes
/// were already read. The future is dropped when this function returns.
///
/// # Errors
/// Returns the first native pipe, status or termination error unchanged.
pub async fn wait_for_child_process<F>(
    child: &mut CapturedChild,
    stderr: StderrMode,
    termination: F,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
{
    let stdout_read = collect(child.stdout.take(), StderrMode::Capture);
    let stderr_read = collect(child.stderr.take(), stderr);
    tokio::pin!(stdout_read, stderr_read, termination);
    let (mut out, mut err, mut status) = (None, None, None);
    let mut stopped = false;
    while out.is_none() || err.is_none() || status.is_none() {
        tokio::select! {
            value = &mut stdout_read, if out.is_none() => out = Some(value?),
            value = &mut stderr_read, if err.is_none() => err = Some(value?),
            value = child.child.wait(), if status.is_none() => status = Some(value?),
            requested = &mut termination, if !stopped => {
                stopped = true;
                child.terminate(requested).await?;
                if requested == Termination::Kill {
                    break;
                }
            }
        }
    }
    let status = match status {
        Some(status) => status,
        None => child.child.wait().await?,
    };
    Ok(Output {
        status,
        stdout: out.unwrap_or_default(),
        stderr: err.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests;
