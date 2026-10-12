//! Captured child processes: spawn, raw pipe collection, exit and EOF observation,
//! termination and reaping.
//!
//! Callers keep cancellation, deadlines, output decoding and error text; a wait
//! applies only the exit grace and termination action its caller selects.
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::time::Sleep;

/// A spawned child whose stdout and stderr are captured and whose stdin is closed.
pub struct CapturedChild {
    /// The native child, reaped by [`CapturedChild::terminate`] with [`Termination::Kill`]
    /// and by [`wait_for_child_process`] once it returns.
    child: Child,
    /// Stdout, until it ends, fails or is dropped.
    stdout: Option<ChildStdout>,
    /// Stderr, until it ends, fails or is dropped.
    stderr: Option<ChildStderr>,
    /// Stdout bytes consumed so far and not yet handed to a caller.
    out: Vec<u8>,
    /// Stderr bytes consumed so far and not yet handed to a caller.
    err: Vec<u8>,
}

/// Selections for one [`wait_for_child_process`] call.
#[derive(Clone, Copy, Debug)]
pub struct WaitOptions {
    /// Whether stderr bytes are returned or consumed and dropped.
    pub stderr: StderrMode,
    /// How long pipes may stay open after the child exits; `None` waits for both
    /// end-of-file events. On expiry the bytes consumed so far are returned and the
    /// pipes are dropped.
    pub post_exit_grace: Option<Duration>,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            stderr: StderrMode::Capture,
            post_exit_grace: None,
        }
    }
}

/// Whether stderr bytes are returned or consumed and dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StderrMode {
    /// Return the stderr bytes the wait collects.
    Capture,
    /// Read stderr and return nothing.
    Discard,
}

/// The native action a caller requests for a running child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    /// Ordinary platform termination, with no wait and no escalation.
    Terminate,
    /// Forced kill of the direct child, awaited until it is reaped.
    Kill,
    /// Ordinary termination, then a forced kill if the child is still running
    /// after `after`.
    TerminateThenKill {
        /// How long the child may take to exit before it is killed.
        after: Duration,
    },
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
            out: Vec::new(),
            err: Vec::new(),
        })
    }

    /// Moves out the bytes consumed so far and drops any open pipes.
    ///
    /// A failed wait leaves what it consumed here; a successful wait has already
    /// returned its bytes in the [`Output`].
    pub fn take_output(&mut self) -> (Vec<u8>, Vec<u8>) {
        self.stdout = None;
        self.stderr = None;
        (std::mem::take(&mut self.out), std::mem::take(&mut self.err))
    }

    /// Applies `termination` to the child if it has not already exited.
    ///
    /// The child's current state decides whether a signal is sent, so a reaped
    /// child is never signalled. [`Termination::Terminate`] ignores native
    /// failures; [`Termination::Kill`] propagates them and returns once the
    /// direct child is reaped. [`Termination::TerminateThenKill`] terminates, then
    /// waits for the exit or the selected delay, then kills a child that is still
    /// running. Descendants and process groups are untouched.
    ///
    /// # Errors
    /// Returns the native status or kill error of a forced kill.
    pub async fn terminate(&mut self, termination: Termination) -> io::Result<()> {
        match termination {
            Termination::Kill => self.kill_if_running().await,
            Termination::Terminate => {
                self.request_termination();
                Ok(())
            }
            Termination::TerminateThenKill { after } => {
                let running = self.request_termination();
                if running
                    && tokio::time::timeout(after, self.child.wait())
                        .await
                        .is_err()
                {
                    self.kill_if_running().await?;
                }
                Ok(())
            }
        }
    }

    /// Starts a wait's termination request without waiting for the exit.
    async fn begin(&mut self, requested: Termination) -> io::Result<Next> {
        Ok(match requested {
            Termination::Kill => {
                self.kill_if_running().await?;
                if self.stdout.is_some() {
                    self.out.clear();
                }
                if self.stderr.is_some() {
                    self.err.clear();
                }
                Next::Abandon
            }
            Termination::Terminate => {
                self.request_termination();
                Next::Resume(None)
            }
            Termination::TerminateThenKill { after } => {
                Next::Resume(self.request_termination().then_some(after))
            }
        })
    }

    /// Kills and reaps the direct child unless it has already exited.
    async fn kill_if_running(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill().await?;
        }
        Ok(())
    }

    /// Sends the ordinary termination signal, or kills where none exists, to a child
    /// that has not exited; returns whether it was still running.
    fn request_termination(&mut self) -> bool {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return false;
        }
        #[cfg(unix)]
        if let Some(pid) = self
            .child
            .id()
            .and_then(|pid| i32::try_from(pid).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
        #[cfg(not(unix))]
        let _ = self.child.start_kill();
        true
    }
}

/// What a wait does once a termination request has started.
enum Next {
    /// Keep collecting; the delay, if any, arms a forced kill.
    Resume(Option<Duration>),
    /// Stop collecting and reap the child.
    Abandon,
}

/// Reads one chunk of `pipe` into `buf`, keeping it only when `keep` is set.
///
/// The pipe is dropped at its end or on error.
async fn fill(
    pipe: &mut Option<impl AsyncRead + Unpin>,
    buf: &mut Vec<u8>,
    keep: bool,
) -> io::Result<()> {
    let Some(reader) = pipe.as_mut() else {
        return std::future::pending().await;
    };
    let start = buf.len();
    match reader.read_buf(buf).await {
        Ok(0) => *pipe = None,
        Ok(_) if !keep => buf.truncate(start),
        Ok(_) => {}
        Err(error) => {
            *pipe = None;
            return Err(error);
        }
    }
    Ok(())
}

/// Resolves when `timer` fires; never resolves while it is unset.
async fn fires(timer: &mut Option<Pin<Box<Sleep>>>) {
    match timer {
        Some(sleep) => sleep.as_mut().await,
        None => std::future::pending().await,
    }
}

/// Waits for the exit and both pipe EOFs, honouring one termination request.
///
/// `termination` is polled until it yields once. [`Termination::Terminate`] and
/// [`Termination::TerminateThenKill`] keep collecting until exit and both EOFs, the
/// latter killing a child still running after its delay. [`Termination::Kill`]
/// abandons readers that have not finished, returns once the direct child is reaped
/// and gives empty bytes for each abandoned stream; completed streams keep theirs.
/// A `post_exit_grace` ends the wait that long after the exit with the bytes
/// consumed so far. The future is dropped when this function returns.
///
/// # Errors
/// Returns the first native pipe, status or termination error unchanged; bytes
/// consumed before it stay available through [`CapturedChild::take_output`].
pub async fn wait_for_child_process<F>(
    child: &mut CapturedChild,
    options: WaitOptions,
    termination: F,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
{
    tokio::pin!(termination);
    let mut status: Option<ExitStatus> = None;
    let mut grace = None;
    let mut escalation = None;
    let mut stopped = false;
    while child.stdout.is_some() || child.stderr.is_some() || status.is_none() {
        tokio::select! {
            biased;
            value = fill(&mut child.stdout, &mut child.out, true), if child.stdout.is_some() => value?,
            value = fill(&mut child.stderr, &mut child.err, options.stderr == StderrMode::Capture), if child.stderr.is_some() => value?,
            value = child.child.wait(), if status.is_none() => {
                status = Some(value?);
                escalation = None;
                grace = options.post_exit_grace.map(|after| Box::pin(tokio::time::sleep(after)));
            }
            () = fires(&mut grace) => break,
            () = fires(&mut escalation) => {
                escalation = None;
                child.kill_if_running().await?;
            }
            requested = &mut termination, if !stopped => {
                stopped = true;
                match child.begin(requested).await? {
                    Next::Resume(after) => {
                        escalation = after.map(|after| Box::pin(tokio::time::sleep(after)));
                    }
                    Next::Abandon => break,
                }
            }
        }
    }
    child.stdout = None;
    child.stderr = None;
    let status = match status {
        Some(status) => status,
        None => child.child.wait().await?,
    };
    let (stdout, stderr) = child.take_output();
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests;
