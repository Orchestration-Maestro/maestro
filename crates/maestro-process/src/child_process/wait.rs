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

/// Delivers newly read bytes and removes only those bytes from retained output.
fn deliver<S: FnMut(OutputStream, &[u8]) + ?Sized>(
    buf: &mut Vec<u8>,
    start: Option<usize>,
    sink: &mut Option<&mut S>,
    stream: OutputStream,
) {
    if let (Some(start), Some(sink)) = (start, sink) {
        sink(stream, &buf[start..]);
        buf.truncate(start);
    }
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
    S: FnMut(OutputStream, &[u8]) + ?Sized,
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
            value = fill(&mut child.stdout, &mut child.out, true), if child.stdout.is_some() => deliver(&mut child.out, value?, &mut on_data, OutputStream::Stdout),
            value = fill(&mut child.stderr, &mut child.err, on_data.is_some() || options.stderr == OutputMode::Capture), if child.stderr.is_some() => deliver(&mut child.err, value?, &mut on_data, OutputStream::Stderr),
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
