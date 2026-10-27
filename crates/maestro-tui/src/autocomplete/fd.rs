//! Native search execution with owned child and captured pipes.
use maestro_cancellation::Cancellation;
use maestro_process::child_process::{
    CapturedChild, OutputMode, Termination, WaitOptions, wait_for_child_process,
};
use std::io;

/// Execute one search, draining both pipes before publishing successful stdout.
pub(super) async fn run(
    executable: &str,
    args: &[String],
    signal: &Cancellation,
) -> io::Result<Vec<u8>> {
    if signal.is_aborted() {
        return Err(cancelled());
    }
    let mut child = CapturedChild::spawn(maestro_process::command(executable).args(args))?;
    let kill = async {
        signal.cancelled().await;
        Termination::Kill
    };
    let options = WaitOptions {
        stderr: OutputMode::Discard,
        ..WaitOptions::default()
    };
    let result = wait_for_child_process(&mut child, options, kill, None).await;
    if result.is_err() {
        child.terminate(Termination::Kill).await?;
    }
    if signal.is_aborted() {
        return Err(cancelled());
    }
    let output = result?;
    if !output.status.success() {
        return Err(io::Error::other("search process failed"));
    }
    Ok(output.stdout)
}
/// Cancellation represented as an interrupted host operation.
fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "search cancelled")
}
