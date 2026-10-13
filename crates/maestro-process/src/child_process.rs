//! Captured child processes: spawn, raw pipe collection, exit and EOF observation,
//! termination and reaping.
//!
//! Callers keep cancellation, output decoding and error text and choose deadlines;
//! waits apply the selected exit grace and termination action.
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::{ExitStatus, Output, Stdio};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::time::Sleep;

mod native;
#[cfg(windows)]
pub use native::request_tree_kill;
#[cfg(unix)]
pub use native::{kill_process, kill_process_group};

/// Default post-exit pipe grace for local shells, in milliseconds.
pub const EXIT_STDIO_GRACE_MS: u64 = 100;

/// A synchronous raw-byte sink for a streaming wait.
pub type OutputSink<'a> = &'a mut (dyn FnMut(&[u8]) + Send);

/// A spawned child whose stdout and stderr are captured and whose stdin is closed.
pub struct CapturedChild {
    /// The native child, reaped once [`CapturedChild::terminate`] with [`Termination::Kill`]
    /// or [`wait_for_child_process`] returns successfully.
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
        Self::spawn_with(command, Stdio::piped())
    }

    /// Returns the native PID while the child has not been reaped.
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Spawns with a new Unix session; other platforms use ordinary spawning.
    ///
    /// # Errors
    /// Returns the native session configuration or spawn error.
    pub fn spawn_detached(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use process_wrap::tokio::{CommandWrap, CommandWrapper, ProcessSession};
            let context = CommandWrap::from(Command::new(""));
            ProcessSession.pre_spawn(command, &context)?;
        }
        Self::spawn(command)
    }

    /// Spawns `command` with ignored stdin, captured stdout and the selected stderr.
    fn spawn_with(command: &mut Command, stderr: Stdio) -> io::Result<Self> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(stderr);
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
    /// A failed wait leaves retained bytes here; a successful wait has already
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

    /// Drops retained stderr bytes when a wait discards stderr.
    fn apply(&mut self, stderr: StderrMode) {
        if stderr == StderrMode::Discard {
            self.err.clear();
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
        if let Some(pid) = self.child.id() {
            let _ = native::signal(pid, rustix::process::Signal::TERM, false);
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
) -> io::Result<Option<usize>> {
    let Some(reader) = pipe.as_mut() else {
        return std::future::pending().await;
    };
    let start = buf.len();
    match reader.read_buf(buf).await {
        Ok(0) => *pipe = None,
        Ok(_) if !keep => buf.truncate(start),
        Ok(_) => return Ok(Some(start)),
        Err(error) => {
            *pipe = None;
            return Err(error);
        }
    }
    Ok(None)
}

/// Delivers newly read bytes and removes only those bytes from retained output.
fn deliver(buf: &mut Vec<u8>, start: Option<usize>, sink: &mut Option<OutputSink<'_>>) {
    if let (Some(start), Some(sink)) = (start, sink) {
        sink(&buf[start..]);
        buf.truncate(start);
    }
}

/// A timer for `after`, if one is selected.
fn timer(after: Option<Duration>) -> Option<Pin<Box<Sleep>>> {
    after.map(|after| Box::pin(tokio::time::sleep(after)))
}

/// Resolves when `timer` fires; never resolves while it is unset.
async fn fires(timer: &mut Option<Pin<Box<Sleep>>>) {
    match timer {
        Some(sleep) => sleep.as_mut().await,
        None => std::future::pending().await,
    }
}

/// Waits for the exit and both pipe EOFs (or the selected grace), honouring one
/// termination request.
///
/// `termination` is polled before any pipe or status, until it yields once, so a
/// request that is already ready is honoured even when the child has finished.
/// [`Termination::Terminate`] and [`Termination::TerminateThenKill`] keep collecting
/// until exit and both EOFs, the latter killing a child still running after its delay.
/// [`Termination::Kill`]
/// abandons readers that have not finished, returns once the direct child is reaped
/// and gives empty bytes for each abandoned stream; completed streams keep theirs.
/// [`StderrMode::Discard`] also drops stderr bytes retained by an earlier wait.
/// A `post_exit_grace` ends the wait that long after the exit with the bytes
/// consumed so far; without one the wait ends at the exit and both end-of-file
/// events. The future is dropped when this function returns.
///
/// With `on_data`, newly consumed chunks go synchronously to the sink instead of
/// retained output. Earlier retained bytes are not replayed.
///
/// # Errors
/// Returns the first native pipe, status or termination error unchanged; bytes
/// retained before it stay available through [`CapturedChild::take_output`].
pub async fn wait_for_child_process<F>(
    child: &mut CapturedChild,
    options: WaitOptions,
    termination: F,
    mut on_data: Option<OutputSink<'_>>,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
{
    tokio::pin!(termination);
    let mut status: Option<ExitStatus> = None;
    let mut grace = None;
    let mut escalation = None;
    let mut stopped = false;
    child.apply(options.stderr);
    while child.stdout.is_some() || child.stderr.is_some() || status.is_none() {
        tokio::select! {
            biased;
            requested = &mut termination, if !stopped => {
                stopped = true;
                let Next::Resume(after) = child.begin(requested).await? else {
                    break;
                };
                escalation = timer(after);
            }
            () = fires(&mut escalation) => {
                escalation = None;
                child.kill_if_running().await?;
            }
            () = fires(&mut grace) => break,
            value = child.child.wait(), if status.is_none() => {
                status = Some(value?);
                escalation = None;
                grace = timer(options.post_exit_grace);
            }
            value = fill(&mut child.stdout, &mut child.out, true), if child.stdout.is_some() => deliver(&mut child.out, value?, &mut on_data),
            value = fill(&mut child.stderr, &mut child.err, on_data.is_some() || options.stderr == StderrMode::Capture), if child.stderr.is_some() => deliver(&mut child.err, value?, &mut on_data),
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

/// Why a synchronous capture produced no completed output.
#[derive(Debug)]
pub enum SpawnSyncError {
    /// The helper thread or its runtime failed.
    Infrastructure(io::Error),
    /// The native spawn failed.
    Spawn(io::Error),
    /// Reading stdout or the exit status failed after the spawn.
    Wait(io::Error),
    /// The timeout expired before the exit and the stdout end of file.
    Timeout,
}

impl std::fmt::Display for SpawnSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Infrastructure(_) => "Cannot start the synchronous capture",
            Self::Spawn(_) => "Cannot start the process",
            Self::Wait(_) => "Cannot wait for the process",
            Self::Timeout => "The process timed out",
        })
    }
}

impl std::error::Error for SpawnSyncError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Infrastructure(error) | Self::Spawn(error) | Self::Wait(error) => Some(error),
            Self::Timeout => None,
        }
    }
}

/// Runs a caller-built `command` from synchronous code, including inside a runtime.
///
/// Keeps its program, arguments, directory, environment and platform flags. Stdin
/// and stderr use the null device; output contains raw stdout and the actual status.
/// The timeout covers exit and stdout EOF. On expiry, unfinished readers close and
/// the direct child is force-killed if still running, then reaped unless killing
/// fails. Descendants are not signalled, and the result is [`SpawnSyncError::Timeout`].
///
/// # Errors
/// Returns [`SpawnSyncError::Infrastructure`] for thread/runtime failure,
/// [`SpawnSyncError::Spawn`] for native spawn failure, or [`SpawnSyncError::Wait`]
/// for an error from [`wait_for_child_process`]. Expiry returns
/// [`SpawnSyncError::Timeout`]; an unsuccessful exit status is not an error.
pub fn spawn_sync(command: &mut Command, timeout: Duration) -> Result<Output, SpawnSyncError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .spawn_scoped(scope, || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(SpawnSyncError::Infrastructure)?
                    .block_on(capture(command, timeout))
            })
            .map_err(SpawnSyncError::Infrastructure)?
            .join()
            .unwrap_or_else(|_| {
                Err(SpawnSyncError::Infrastructure(io::Error::other(
                    "synchronous capture thread panicked",
                )))
            })
    })
}

/// Captures one child on the current runtime until exit and stdout end of file or `timeout`.
async fn capture(command: &mut Command, timeout: Duration) -> Result<Output, SpawnSyncError> {
    command.kill_on_drop(true);
    let mut child =
        CapturedChild::spawn_with(command, Stdio::null()).map_err(SpawnSyncError::Spawn)?;
    let mut expired = false;
    let output = wait_for_child_process(
        &mut child,
        WaitOptions::default(),
        async {
            tokio::time::sleep(timeout).await;
            expired = true;
            Termination::Kill
        },
        None,
    )
    .await;
    if expired {
        return Err(SpawnSyncError::Timeout);
    }
    output.map_err(SpawnSyncError::Wait)
}

#[cfg(test)]
mod tests;
