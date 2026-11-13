//! The child's native command and the tasks that own its streams and its lifetime.
use super::{Inner, RpcClientOptions, Running};
use crate::connection_runner::{JsonlLineReader, RpcInput, RpcLineReader};
use maestro_process::children::{CapturedChild, Termination};
use maestro_text::decoding::Utf8Stream;
use std::{
    io::{self, Write},
    path::Path,
    pin::Pin,
    process::ExitStatus,
    rc::{Rc, Weak},
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    io::AsyncWriteExt,
    process::{ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{mpsc, oneshot, watch},
    time::Sleep,
};

/// How long a new child has to exit for its start to report the exit.
const STARTUP_OBSERVATION: Duration = Duration::from_millis(100);

/// How long a stopped child may take to exit before it is killed.
const STOP_GRACE: Duration = Duration::from_millis(1000);

/// The command that launches `options`' entry in RPC mode, without a shell. A relative
/// entry is resolved against the child's working directory.
///
/// # Errors
/// Returns the failure to read the current directory for a relative path.
pub(super) fn command_for(options: &RpcClientOptions) -> io::Result<Command> {
    let default = format!("dist/maestro{}", std::env::consts::EXE_SUFFIX);
    let entry = options.cli_path.as_deref().unwrap_or(&default);
    let base = Path::new(options.cwd.as_deref().unwrap_or_default());
    let mut command = maestro_process::command(std::path::absolute(base.join(entry))?);
    command.args(["--mode", "rpc"]);
    for (flag, value) in [
        ("--provider", &options.provider),
        ("--model", &options.model),
    ] {
        if let Some(value) = value.as_deref().filter(|value| !value.is_empty()) {
            command.args([flag, value]);
        }
    }
    command.args(&options.args).envs(&options.env);
    if let Some(cwd) = &options.cwd {
        command.current_dir(cwd);
    }
    Ok(command)
}

/// Starts the diagnostics' forwarding with `forward`, launches `command`, records the
/// child as the running child and starts the tasks that read its streams and await its
/// end; returns the receiver of the child's start-up outcome, as `Startup` describes it.
/// A failed step publishes nothing and leaves no child running.
///
/// # Errors
/// Returns `forward`'s failure, the native launch failure, or a broken-pipe failure for
/// a child spawned without its three pipes.
pub(super) async fn launch(
    inner: &Rc<Inner>,
    command: &mut Command,
    forward: impl FnOnce() -> io::Result<std::sync::mpsc::Sender<Forward>>,
) -> io::Result<oneshot::Receiver<Option<ExitStatus>>> {
    let forward = forward()?;
    let mut child = CapturedChild::spawn_piped(command)?;
    let window = Box::pin(tokio::time::sleep(STARTUP_OBSERVATION));
    let pipes = (child.take_stdin(), child.take_stdout(), child.take_stderr());
    let (Some(input), Some(output), Some(errors)) = pipes else {
        child.terminate(Termination::Kill).await?;
        return Err(io::ErrorKind::BrokenPipe.into());
    };
    let (lines, queue) = mpsc::unbounded_channel();
    let (stop, stopped) = oneshot::channel();
    let (finished, done) = watch::channel(());
    let (ended, diagnostics) = oneshot::channel();
    let (report, outcome) = oneshot::channel();
    inner.state.borrow_mut().running = Some(Running {
        input: lines,
        stop: Some(stop),
        done,
        forward: forward.clone(),
    });
    let ends = ClientEnds {
        stopped,
        startup: Some(Startup {
            window,
            diagnostics,
            report,
        }),
        finished,
    };
    let weak = Rc::downgrade(inner);
    tokio::task::spawn_local(write_lines(input, queue, weak.clone()));
    tokio::task::spawn_local(record_stderr(errors, forward, weak.clone(), ended));
    tokio::task::spawn_local(drive_child(child, output, ends, weak));
    Ok(outcome)
}

/// The start-up outcome a driver owes its client: the child's exit status once the child
/// has exited and its diagnostics have ended, or when the window ends after an exit
/// while a descendant holds them open; `None` for a child still running when the window
/// ends.
struct Startup {
    /// The end of the start-up window.
    window: Pin<Box<Sleep>>,
    /// Settles once the child's diagnostic stream has ended.
    diagnostics: oneshot::Receiver<()>,
    /// Takes the outcome.
    report: oneshot::Sender<Option<ExitStatus>>,
}

/// The driver's ends of the channels it shares with its client.
struct ClientEnds {
    /// Takes a stop request, with the sender for its outcome.
    stopped: oneshot::Receiver<oneshot::Sender<io::Result<()>>>,
    /// The start-up outcome until it is reported.
    startup: Option<Startup>,
    /// Dropped when the driver returns.
    finished: watch::Sender<()>,
}

/// Writes each queued request line to the child's input, settling a request whose write
/// fails, until the client drops its queue.
async fn write_lines(
    mut input: ChildStdin,
    mut queue: mpsc::UnboundedReceiver<(String, String)>,
    inner: Weak<Inner>,
) {
    while let Some((line, id)) = queue.recv().await {
        if let Err(error) = input.write_all(line.as_bytes()).await
            && let Some(inner) = inner.upgrade()
        {
            inner.fail_write(&id, error);
        }
    }
}

/// What the forwarding thread handles, in order.
pub(super) enum Forward {
    /// Diagnostic bytes for this process's stderr.
    Chunk(Vec<u8>),
    /// Answered once every chunk queued before it has been written.
    Flush(oneshot::Sender<()>),
}

/// Starts the thread that writes the child's diagnostic bytes to this process's stderr, so
/// a slow reader of it never holds up the client; the thread ends once every sender is
/// gone.
///
/// # Errors
/// Returns the failure to start the thread.
pub(super) fn forward_stderr() -> io::Result<std::sync::mpsc::Sender<Forward>> {
    let (forward, queue) = std::sync::mpsc::channel();
    std::thread::Builder::new().spawn(move || {
        let mut stderr = io::stderr();
        for item in queue {
            match item {
                // Like any diagnostic output, a failure to forward is not the child's failure.
                Forward::Chunk(chunk) => {
                    let _ = stderr.write_all(&chunk);
                }
                Forward::Flush(written) => {
                    let _ = written.send(());
                }
            }
        }
    })?;
    Ok(forward)
}

/// Waits until the forwarding thread has written every chunk queued before this call; at
/// once when the thread has gone.
pub(super) async fn flush(forward: &std::sync::mpsc::Sender<Forward>) {
    let (flushed, written) = oneshot::channel();
    if forward.send(Forward::Flush(flushed)).is_ok() {
        let _ = written.await;
    }
}

/// Appends the text of the child's diagnostic bytes to the client's, then hands the bytes
/// to `forward`, until the pipe ends; then reports that end through `ended`.
async fn record_stderr(
    mut errors: ChildStderr,
    forward: std::sync::mpsc::Sender<Forward>,
    inner: Weak<Inner>,
    ended: oneshot::Sender<()>,
) {
    let mut text = Utf8Stream::default();
    loop {
        let mut chunk = Vec::with_capacity(8192);
        if !matches!(errors.read_buf(&mut chunk).await, Ok(1..)) {
            break;
        }
        append_stderr(&inner, &text.push(&chunk));
        // A send fails only once the forwarding thread has gone, leaving nowhere to forward.
        let _ = forward.send(Forward::Chunk(chunk));
    }
    append_stderr(&inner, &text.finish());
    // A send fails only once the driver has stopped waiting for this end.
    let _ = ended.send(());
}

/// Appends `text` to the client's diagnostic text while the client exists.
fn append_stderr(inner: &Weak<Inner>, text: &str) {
    if let Some(inner) = inner.upgrade() {
        inner.state.borrow_mut().stderr.push_str(text);
    }
}

/// Hands each output line to the client until a stop request or until the child has exited,
/// its output has ended and its start-up outcome is reported. The child's exit ends the
/// client's pending work, whoever still holds the output open, and so does the driver's end,
/// for work added meanwhile. The start-up outcome comes before output, so a busy output
/// never delays it, and it asks the system for an exit the driver has not handled yet. A
/// stop detaches the output, then terminates the child and reports the outcome, leaving the
/// start-up outcome unreported. The client's ends are dropped on return.
async fn drive_child(
    mut child: CapturedChild,
    output: ChildStdout,
    ends: ClientEnds,
    inner: Weak<Inner>,
) {
    let ClientEnds {
        stopped,
        mut startup,
        finished,
    } = ends;
    let mut lines = JsonlLineReader::new(output);
    let (mut reading, mut exited, mut status) = (true, false, None);
    let stop = async {
        match stopped.await {
            Ok(report) => report,
            Err(_) => std::future::pending().await,
        }
    };
    tokio::pin!(stop);
    while reading || !exited || startup.is_some() {
        tokio::select! {
            biased;
            report = &mut stop => {
                lines.detach();
                let _ = report.send(child.terminate(Termination::TerminateThenKill { after: STOP_GRACE }).await);
                break;
            }
            () = settled(&mut startup, exited) => {
                if let Some(owed) = startup.take() {
                    // A status the system cannot report leaves the child counted as running.
                    let exit = status.or_else(|| child.try_wait().ok().flatten());
                    // A send fails only once the start awaiting it has been dropped.
                    let _ = owed.report.send(exit);
                }
            }
            read = lines.next_line(), if reading => reading = deliver(read, &inner),
            waited = child.wait(), if !exited => {
                exited = true;
                status = waited.ok();
                settle(&inner);
            }
        }
    }
    settle(&inner);
    drop(finished);
}

/// Hands the line `read` carries to the client while the client exists; returns whether
/// the output continues.
fn deliver(read: io::Result<RpcInput>, inner: &Weak<Inner>) -> bool {
    let (line, reading) = match read {
        Ok(RpcInput::Line(line)) => (Some(line), true),
        Ok(RpcInput::End { tail }) => (tail, false),
        Err(_) => (None, false),
    };
    if let Some(line) = line
        && let Some(inner) = inner.upgrade()
    {
        inner.handle_line(&line);
    }
    reading
}

/// Settles when `startup`'s window ends or, once the child has `exited`, when its
/// diagnostics have ended; never while no outcome is owed. It polls both by reference, so
/// dropping it loses neither.
async fn settled(startup: &mut Option<Startup>, exited: bool) {
    let Some(owed) = startup else {
        return std::future::pending().await;
    };
    tokio::select! {
        () = owed.window.as_mut() => {}
        _ = &mut owed.diagnostics, if exited => {}
    }
}

/// Ends the client's pending work while the client exists.
fn settle(inner: &Weak<Inner>) {
    if let Some(inner) = inner.upgrade() {
        inner.fail_owned();
    }
}
