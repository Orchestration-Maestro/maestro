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
    let mut command = Command::new(std::path::absolute(base.join(entry))?);
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

/// Records `child` as the running child and starts the tasks that read its streams and
/// await its end; returns the receiver that closes once its driver finishes.
///
/// # Errors
/// Returns a broken-pipe failure for a child spawned without its three pipes.
pub(super) fn launch(
    inner: &Rc<Inner>,
    mut child: CapturedChild,
) -> io::Result<watch::Receiver<()>> {
    let pipes = (child.take_stdin(), child.take_stdout(), child.take_stderr());
    let (Some(input), Some(output), Some(errors)) = pipes else {
        return Err(io::ErrorKind::BrokenPipe.into());
    };
    let (lines, queue) = mpsc::unbounded_channel();
    let (stop, stopped) = oneshot::channel();
    let (finished, done) = watch::channel(());
    inner.state.borrow_mut().running = Some(Running {
        input: lines,
        stop: Some(stop),
        done: done.clone(),
        exit: None,
    });
    let forward = forward_stderr()?;
    let weak = Rc::downgrade(inner);
    tokio::task::spawn_local(write_lines(input, queue));
    tokio::task::spawn_local(record_stderr(errors, forward, weak.clone()));
    tokio::task::spawn_local(drive_child(child, output, stopped, weak, finished));
    Ok(done)
}

/// Writes each queued line to the child's input and reports the outcome, until the client
/// drops its queue.
async fn write_lines(
    mut input: ChildStdin,
    mut queue: mpsc::UnboundedReceiver<(String, oneshot::Sender<io::Result<()>>)>,
) {
    while let Some((line, report)) = queue.recv().await {
        let _ = report.send(input.write_all(line.as_bytes()).await);
    }
}

/// Starts the thread that writes the child's diagnostic bytes to this process's stderr, so
/// a slow reader of it never holds up the client; the thread ends with the returned sender.
///
/// # Errors
/// Returns the failure to start the thread.
fn forward_stderr() -> io::Result<std::sync::mpsc::Sender<Vec<u8>>> {
    let (forward, chunks) = std::sync::mpsc::channel::<Vec<u8>>();
    std::thread::Builder::new().spawn(move || {
        let mut stderr = io::stderr();
        for chunk in chunks {
            // Like any diagnostic output, a failure to forward is not the child's failure.
            let _ = stderr.write_all(&chunk);
        }
    })?;
    Ok(forward)
}

/// Appends the text of the child's diagnostic bytes to the client's, then hands the bytes
/// to `forward`, until the pipe ends.
async fn record_stderr(
    mut errors: ChildStderr,
    forward: std::sync::mpsc::Sender<Vec<u8>>,
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
        let _ = forward.send(chunk);
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
