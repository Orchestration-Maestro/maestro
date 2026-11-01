//! The child's native command and the tasks that own its streams and its lifetime.
use super::{Inner, RpcClientOptions, Running};
use crate::rpc_mode::{JsonlLineReader, RpcInput, RpcLineReader};
use maestro_process::child_process::{CapturedChild, Termination};
use maestro_text::utf8::Utf8Stream;
use std::{
    io::{self, Write},
    path::Path,
    rc::{Rc, Weak},
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    io::AsyncWriteExt,
    process::{ChildStderr, ChildStdin, ChildStdout, Command},
    sync::{mpsc, oneshot, watch},
};

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
/// end; returns the receiver that closes once its driver finishes. A failed step
/// publishes nothing and leaves no child running.
///
/// # Errors
/// Returns `forward`'s failure, the native launch failure, or a broken-pipe failure for
/// a child spawned without its three pipes.
pub(super) async fn launch(
    inner: &Rc<Inner>,
    command: &mut Command,
    forward: impl FnOnce() -> io::Result<std::sync::mpsc::Sender<Forward>>,
) -> io::Result<watch::Receiver<()>> {
    let forward = forward()?;
    let mut child = CapturedChild::spawn_piped(command)?;
    let pipes = (child.take_stdin(), child.take_stdout(), child.take_stderr());
    let (Some(input), Some(output), Some(errors)) = pipes else {
        child.terminate(Termination::Kill).await?;
        return Err(io::ErrorKind::BrokenPipe.into());
    };
    let (lines, queue) = mpsc::unbounded_channel();
    let (stop, stopped) = oneshot::channel();
    let (finished, done) = watch::channel(());
    inner.state.borrow_mut().running = Some(Running {
        input: lines,
        stop: Some(stop),
        done: done.clone(),
        forward: forward.clone(),
        exit: None,
    });
    let weak = Rc::downgrade(inner);
    tokio::task::spawn_local(write_lines(input, queue, weak.clone()));
    tokio::task::spawn_local(record_stderr(errors, forward, weak.clone()));
    tokio::task::spawn_local(drive_child(child, output, stopped, weak, finished));
    Ok(done)
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
/// to `forward`, until the pipe ends.
async fn record_stderr(
    mut errors: ChildStderr,
    forward: std::sync::mpsc::Sender<Forward>,
    inner: Weak<Inner>,
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
}

/// Appends `text` to the client's diagnostic text while the client exists.
fn append_stderr(inner: &Weak<Inner>, text: &str) {
    if let Some(inner) = inner.upgrade() {
        inner.state.borrow_mut().stderr.push_str(text);
    }
}

/// Hands each output line to the client until a stop request or until the child has exited
/// and its output has ended. The child's exit ends the client's pending work, whoever still
/// holds the output open, and so does the driver's end, for work added meanwhile. A stop
/// detaches the output, then terminates the child and reports the outcome. `finished` is
/// dropped on return.
async fn drive_child(
    mut child: CapturedChild,
    output: ChildStdout,
    stopped: oneshot::Receiver<oneshot::Sender<io::Result<()>>>,
    inner: Weak<Inner>,
    finished: watch::Sender<()>,
) {
    let mut lines = JsonlLineReader::new(output);
    let (mut reading, mut exited) = (true, false);
    let stop = async {
        match stopped.await {
            Ok(report) => report,
            Err(_) => std::future::pending().await,
        }
    };
    tokio::pin!(stop);
    while reading || !exited {
        tokio::select! {
            biased;
            report = &mut stop => {
                lines.detach();
                let _ = report.send(child.terminate(Termination::TerminateThenKill { after: STOP_GRACE }).await);
                break;
            }
            read = lines.next_line(), if reading => {
                let line = match read {
                    Ok(RpcInput::Line(line)) => Some(line),
                    Ok(RpcInput::End { tail }) => {
                        reading = false;
                        tail
                    }
                    Err(_) => {
                        reading = false;
                        None
                    }
                };
                if let Some(line) = line
                    && let Some(inner) = inner.upgrade()
                {
                    inner.handle_line(&line);
                }
            }
            status = child.wait(), if !exited => {
                exited = true;
                if let Ok(status) = status {
                    record_exit(&inner, status);
                }
                settle(&inner);
            }
        }
    }
    settle(&inner);
    drop(finished);
}

/// Ends the client's pending work while the client exists.
fn settle(inner: &Weak<Inner>) {
    if let Some(inner) = inner.upgrade() {
        inner.fail_owned();
    }
}

/// Records the child's exit status on the running child.
fn record_exit(inner: &Weak<Inner>, status: std::process::ExitStatus) {
    if let Some(inner) = inner.upgrade()
        && let Some(running) = inner.state.borrow_mut().running.as_mut()
    {
        running.exit = Some(status);
    }
}
