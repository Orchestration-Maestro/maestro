//! Shared borrowed-sink capture loop for native child processes.
use super::fill;
use super::{
    CapturedChild, Next, OutputMode, OutputStream, Termination, WaitOptions, fires, timer,
};
use std::{
    future::Future,
    io,
    process::{ExitStatus, Output},
};

/// Observations made by the single native collector, not by a second waiter.
pub(super) trait Observer {
    /// Delivers unmodified bytes from one descriptor.
    fn data(&mut self, stream: OutputStream, bytes: &[u8]);
    /// Publishes this descriptor's EOF independently of child exit.
    fn closed(&mut self, _stream: OutputStream) {}
    /// Hands an error to its consuming resource; true means that resource owns it.
    fn failed(&mut self, _stream: OutputStream, _error: &io::Error) -> bool {
        false
    }
    /// Publishes exit before inherited output pipes finish.
    fn exited(&mut self, _status: ExitStatus) {}
}
impl<S: FnMut(OutputStream, &[u8]) + ?Sized> Observer for S {
    fn data(&mut self, stream: OutputStream, bytes: &[u8]) {
        self(stream, bytes);
    }
}

/// Delivers newly read bytes and removes only those bytes from retained output.
fn deliver<S: Observer + ?Sized>(
    buf: &mut Vec<u8>,
    start: io::Result<Option<usize>>,
    sink: &mut Option<&mut S>,
    stream: OutputStream,
) -> io::Result<()> {
    let start = match start {
        Ok(start) => start,
        Err(error) => {
            if sink
                .as_mut()
                .is_some_and(|sink| sink.failed(stream, &error))
            {
                return Ok(());
            }
            return Err(error);
        }
    };
    if let Some(sink) = sink {
        if let Some(start) = start {
            sink.data(stream, &buf[start..]);
            buf.truncate(start);
        } else {
            sink.closed(stream);
        }
    }
    Ok(())
}

/// Shared capture and delivery loop for both borrowed sink interfaces.
pub(super) async fn wait<F, S>(
    child: &mut CapturedChild,
    options: WaitOptions,
    termination: F,
    mut on_data: Option<&mut S>,
) -> io::Result<Output>
where
    F: Future<Output = Termination>,
    S: Observer + ?Sized,
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
                status = Some(observe_exit(&mut on_data, value)?);
                escalation = None;
                grace = timer(options.post_exit_grace);
            }
            value = fill(&mut child.stdout, &mut child.out, true), if child.stdout.is_some() => deliver(&mut child.out, value, &mut on_data, OutputStream::Stdout)?,
            value = fill(&mut child.stderr, &mut child.err, on_data.is_some() || options.stderr == OutputMode::Capture), if child.stderr.is_some() => deliver(&mut child.err, value, &mut on_data, OutputStream::Stderr)?,
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

/// Publishes a native exit without adding a second status waiter.
fn observe_exit<S: Observer + ?Sized>(
    sink: &mut Option<&mut S>,
    status: io::Result<ExitStatus>,
) -> io::Result<ExitStatus> {
    let status = status?;
    if let Some(sink) = sink {
        sink.exited(status);
    }
    Ok(status)
}
