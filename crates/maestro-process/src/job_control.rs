//! Stopping the current process group and observing its continuation.
use rustix::process::Signal;
use std::{future::Future, io};

/// Sends the terminal stop signal to every process in the current process group, as the
/// terminal's suspend key does.
///
/// # Errors
/// Returns the native signalling failure.
pub fn stop_process_group() -> io::Result<()> {
    rustix::process::kill_current_process_group(Signal::TSTP).map_err(io::Error::from)
}

/// Registers delivery of the continue signal at this call; the returned future completes
/// at the first continuation after it, and dropping the future ends the delivery.
///
/// # Errors
/// Returns the native handler registration failure.
///
/// # Panics
/// Panics outside a Tokio runtime with I/O enabled, as Tokio's signal registration does.
pub fn continued() -> io::Result<impl Future<Output = ()>> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut continuations = signal(SignalKind::from_raw(Signal::CONT.as_raw()))?;
    Ok(async move {
        continuations.recv().await;
    })
}
