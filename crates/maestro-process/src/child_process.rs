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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::time::Sleep;

mod exit_signal;
mod native;
mod raw;
pub use raw::{RawChild, RawExitStatus, RawInput, RawOutput};
mod wait;
pub use exit_signal::exit_signal_name;
#[cfg(windows)]
pub use native::request_tree_kill;
pub use native::signal_pid;
#[cfg(unix)]
pub use native::{Signal, kill_process, kill_process_group, process_exists, signal_processes};
use wait::wait;

/// Default post-exit pipe grace for local shells, in milliseconds.
pub const EXIT_STDIO_GRACE_MS: u64 = 100;

/// Descriptor that produced a raw output chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// A synchronous descriptor-tagged raw-byte sink for a streaming wait.
pub type OutputSink<'a> = &'a mut (dyn FnMut(OutputStream, &[u8]) + Send);

/// A spawned child with selected standard descriptors.
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
    /// Capture-only stderr policy; see [`wait_for_child_process`] for streaming.
    pub stderr: OutputMode,
    /// How long pipes may stay open after the child exits; `None` waits for both
    /// end-of-file events. On expiry retained bytes are returned and the
    /// pipes are dropped.
    pub post_exit_grace: Option<Duration>,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            stderr: OutputMode::Capture,
            post_exit_grace: None,
        }
    }
}

/// Standard-output descriptor selection for synchronous launches; stderr retention
/// policy for waits. See [`wait_for_child_process`] for streaming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputMode {
    /// Pipe synchronous output; retain wait stderr when no streaming sink is supplied.
    Capture,
    /// Use null for synchronous output; drop retained wait stderr and, without a
    /// sink, newly consumed stderr.
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
        Self::spawn_with(command, [Stdio::null(), Stdio::piped(), Stdio::piped()])
    }

    /// Spawns `command` with all three standard streams piped, for a caller that
    /// drives them through the `take_*` methods.
    ///
    /// # Errors
    /// Returns the native spawn error unchanged.
    pub fn spawn_piped(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with(command, [Stdio::piped(), Stdio::piped(), Stdio::piped()])
    }

    /// Moves out the input pipe; `None` once taken or when stdin is not piped.
    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    /// Moves out the output pipe; a later wait then sees no stdout.
    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.stdout.take()
    }

    /// Moves out the error pipe; a later wait then sees no stderr.
    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.stderr.take()
    }

    /// Returns the exit status if the child has exited, reaping it, without waiting.
    ///
    /// # Errors
    /// Returns the native status error unchanged.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    /// Waits for the child to exit and reaps it, first closing an untaken stdin pipe.
    ///
    /// # Errors
    /// Returns the native status error unchanged.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    /// Returns the native PID while the child has not been reaped.
    #[must_use]
    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    /// Consumes `command` and spawns with a new Unix session; other platforms
    /// use ordinary spawning. The session hook cannot affect a later spawn.
    ///
    /// ```compile_fail,E0382
    /// use maestro_process::child_process::CapturedChild;
    /// let command = tokio::process::Command::new("sh");
    /// let first = CapturedChild::spawn_detached(command);
    /// let second = CapturedChild::spawn_detached(command);
    /// ```
    ///
    /// # Errors
    /// Returns the native session configuration or spawn error.
    pub fn spawn_detached(command: Command) -> io::Result<Self> {
        Self::spawn_with_stdio(
            command,
            [Stdio::null(), Stdio::piped(), Stdio::piped()],
            true,
        )
    }

    /// Spawns with supplied descriptors and optional native session creation.
    ///
    /// # Errors
    /// Returns the original native configuration or spawn failure.
    pub fn spawn_with_stdio(
        mut command: Command,
        stdio: [Stdio; 3],
        detached: bool,
    ) -> io::Result<Self> {
        #[cfg(unix)]
        if detached {
            use process_wrap::tokio::{CommandWrap, CommandWrapper, ProcessSession};
            let context = CommandWrap::from(Command::new(""));
            ProcessSession.pre_spawn(&mut command, &context)?;
        }
        #[cfg(not(unix))]
        let _ = detached;
        Self::spawn_with(&mut command, stdio)
    }

    /// Spawns `command` with the selected descriptors.
    fn spawn_with(command: &mut Command, [stdin, stdout, stderr]: [Stdio; 3]) -> io::Result<Self> {
        command.stdin(stdin).stdout(stdout).stderr(stderr);
        let mut child = command.spawn()?;
        Ok(Self {
            stdout: child.stdout.take(),
            stderr: child.stderr.take(),
            child,
            out: Vec::new(),
            err: Vec::new(),
        })
    }

    /// Moves out retained bytes and drops any open pipes.
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
    fn apply(&mut self, stderr: OutputMode) {
        if stderr == OutputMode::Discard {
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
/// and gives empty bytes for each abandoned stream; completed streams keep retained bytes.
/// [`OutputMode::Discard`] also drops stderr bytes retained by an earlier wait.
/// A `post_exit_grace` ends the wait that long after the exit with retained
/// bytes; without one the wait ends at the exit and both end-of-file
/// events. The future is dropped when this function returns.
///
/// With `on_data`, newly consumed chunks go synchronously to the sink instead of
/// retained output, including stderr regardless of [`OutputMode`]. Earlier
/// retained bytes are not replayed.
///
/// # Errors
/// Returns the first native pipe, status or termination error unchanged; bytes
/// retained before it stay available through [`CapturedChild::take_output`].
pub async fn wait_for_child_process<F>(
    child: &mut CapturedChild,
    options: WaitOptions,
    termination: F,
    on_data: Option<OutputSink<'_>>,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
{
    wait(child, options, termination, on_data).await
}

/// Wait with a borrowed raw sink that may retain caller-local state.
///
/// Uses the same retention, termination and descriptor ordering as
/// [`wait_for_child_process`], without requiring a thread-safe sink.
///
/// # Errors
/// Returns the original native pipe, status or termination error.
pub async fn wait_for_child_process_with_sink<F, S>(
    child: &mut CapturedChild,
    options: WaitOptions,
    termination: F,
    on_data: &mut S,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
    S: FnMut(OutputStream, &[u8]) + ?Sized,
{
    wait(child, options, termination, Some(on_data)).await
}

/// Why a synchronous capture produced no completed output.
#[derive(Debug)]
pub enum SpawnSyncError {
    /// The helper thread or its runtime failed.
    Infrastructure(io::Error),
    /// The native spawn failed.
    Spawn(io::Error),
    /// Input transfer, pipe collection or exit observation failed after the spawn.
    Wait(io::Error),
    /// The deadline expired before input, exit and captured pipe EOFs completed.
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

/// Descriptor and deadline selection for [`spawn_sync`].
#[derive(Clone, Copy, Debug)]
pub enum SpawnSyncOptions<'a> {
    /// Select raw output descriptors with optional input and deadline.
    Capture {
        /// Whether stdout is captured or sent to the null device.
        stdout: OutputMode,
        /// Bytes written unchanged, followed by EOF; absence selects null stdin.
        input: Option<&'a [u8]>,
        /// Deadline for input, exit and captured EOFs; None and zero disable it.
        timeout: Option<Duration>,
        /// Whether stderr is captured or sent to the null device.
        stderr: OutputMode,
        /// Combined captured byte limit; None and zero disable it.
        max_buffer_bytes: Option<usize>,
    },
    /// Inherit all three descriptors and return status with empty captured streams.
    Inherit,
}

/// Constructs the current-thread runtime shared by blocking launch bridges.
pub(crate) fn runtime() -> io::Result<tokio::runtime::Runtime> {
    runtime_builder().build()
}

/// Configures the single current-thread runtime owner before construction.
pub(crate) fn runtime_builder() -> tokio::runtime::Builder {
    let mut builder = tokio::runtime::Builder::new_current_thread();
    builder.enable_all();
    builder
}

/// Writes supplied bytes and closes the owned pipe, including on failure.
async fn write_input(stdin: Option<ChildStdin>, input: Option<&[u8]>) -> io::Result<()> {
    if let (Some(mut stdin), Some(input)) = (stdin, input) {
        stdin.write_all(input).await?;
    }
    Ok(())
}

/// Collects input and output concurrently, retaining write failures until exit/EOF.
async fn complete(child: &mut CapturedChild, input: Option<&[u8]>) -> io::Result<Output> {
    settle_input(
        write_input(child.child.stdin.take(), input),
        wait_for_child_process(child, WaitOptions::default(), std::future::pending(), None),
    )
    .await
}

/// Collects bounded output through the existing streaming wait and termination path.
async fn complete_capture(
    child: &mut CapturedChild,
    input: Option<&[u8]>,
    limit: Option<usize>,
) -> io::Result<Output> {
    let Some(limit) = limit else {
        return complete(child, input).await;
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let mut sender = Some(sender);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut exceeded = false;
    let cancel_input = tokio::sync::Notify::new();
    let mut collect = |stream, bytes: &[u8]| {
        if exceeded {
            return;
        }
        if bytes.len() > limit - stdout.len() - stderr.len() {
            exceeded = true;
            cancel_input.notify_one();
            if let Some(sender) = sender.take() {
                let _ = sender.send(());
            }
        } else {
            match stream {
                OutputStream::Stdout => stdout.extend_from_slice(bytes),
                OutputStream::Stderr => stderr.extend_from_slice(bytes),
            }
        }
    };
    let stdin = child.child.stdin.take();
    let written = async {
        tokio::select! {
            result = write_input(stdin, input) => result,
            () = cancel_input.notified() => Ok(()),
        }
    };
    let stopped = async {
        let _ = receiver.await;
        Termination::Kill
    };
    let result = settle_input(
        written,
        wait_for_child_process(child, WaitOptions::default(), stopped, Some(&mut collect)),
    )
    .await;
    let mut output = result?;
    if exceeded {
        return Err(io::Error::other("captured output exceeds byte limit"));
    }
    output.stdout = stdout;
    output.stderr = stderr;
    Ok(output)
}

/// Retains input failure until output collection settles.
async fn settle_input(
    written: impl Future<Output = io::Result<()>>,
    output: impl Future<Output = io::Result<Output>>,
) -> io::Result<Output> {
    let (written, output) = tokio::join!(written, output);
    written?;
    output
}

/// Runs a caller-built command from synchronous code, including inside a runtime.
///
/// Preserves command operands and platform settings. No shell is selected.
/// A capture deadline covers input, exit and captured EOFs; on expiry unfinished
/// pipe futures close and the direct child is force-killed/reaped unless cleanup fails.
/// Descendants are not signalled. See [`SpawnSyncOptions`] for descriptor choices.
///
/// # Errors
/// Returns native spawn/wait failures or bridge failures in [`SpawnSyncError`].
/// Expiry returns [`SpawnSyncError::Timeout`]; nonzero exit is a completed output.
pub fn spawn_sync(
    command: &mut Command,
    options: SpawnSyncOptions<'_>,
) -> Result<Output, SpawnSyncError> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .spawn_scoped(scope, || {
                runtime()
                    .map_err(SpawnSyncError::Infrastructure)?
                    .block_on(capture(command, options))
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

/// Captures one child with the selected descriptors and combined deadline.
async fn capture(
    command: &mut Command,
    options: SpawnSyncOptions<'_>,
) -> Result<Output, SpawnSyncError> {
    let (descriptors, input, timeout, max_buffer_bytes) = match options {
        SpawnSyncOptions::Inherit => (
            [Stdio::inherit(), Stdio::inherit(), Stdio::inherit()],
            None,
            None,
            None,
        ),
        SpawnSyncOptions::Capture {
            stdout,
            input,
            timeout,
            stderr,
            max_buffer_bytes,
        } => (
            [
                if input.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                },
                if stdout == OutputMode::Capture {
                    Stdio::piped()
                } else {
                    Stdio::null()
                },
                if stderr == OutputMode::Capture {
                    Stdio::piped()
                } else {
                    Stdio::null()
                },
            ],
            input,
            timeout.filter(|duration| !duration.is_zero()),
            max_buffer_bytes.filter(|limit| *limit != 0),
        ),
    };
    command.kill_on_drop(true);
    let mut child =
        CapturedChild::spawn_with(command, descriptors).map_err(SpawnSyncError::Spawn)?;
    let mut deadline = timer(timeout);
    tokio::select! {
        biased;
        () = fires(&mut deadline) => {}
        result = complete_capture(&mut child, input, max_buffer_bytes) => return result.map_err(SpawnSyncError::Wait),
    }
    child.take_output();
    child
        .terminate(Termination::Kill)
        .await
        .map_err(SpawnSyncError::Wait)?;
    Err(SpawnSyncError::Timeout)
}

/// Starts a caller-built command in a new Unix session, then transfers input and waits in an
/// owned worker. The command leaves the caller's process group, so a stop signal sent to that
/// group, such as a terminal suspend, does not reach it.
///
/// Stdout/stderr use the null device. Input errors are ignored; normal waiting
/// reaps the direct child. Dropping the handle neither joins nor cancels the worker.
///
/// # Errors
/// Returns native startup or worker/runtime construction errors unchanged.
pub fn spawn(command: Command, input: Option<Vec<u8>>) -> io::Result<std::thread::JoinHandle<()>> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(0);
    let worker = std::thread::Builder::new().spawn(move || {
        let started = runtime().and_then(|runtime| {
            let guard = runtime.enter();
            let child = CapturedChild::spawn_with_stdio(
                command,
                [
                    if input.is_some() {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    },
                    Stdio::null(),
                    Stdio::null(),
                ],
                true,
            )?;
            drop(guard);
            Ok((runtime, child))
        });
        match started {
            Ok((runtime, mut child)) => {
                if sender.send(Ok(())).is_ok() {
                    let _ = runtime.block_on(complete(&mut child, input.as_deref()));
                }
            }
            Err(error) => {
                let _ = sender.send(Err(error));
            }
        }
    })?;
    match receiver.recv().map_err(io::Error::other)? {
        Ok(()) => Ok(worker),
        Err(error) => {
            let _ = worker.join();
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
