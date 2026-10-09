//! Native search execution with owned child and captured pipes.
use maestro_cancellation::Cancellation;
use std::io;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

/// Execute one search, draining both pipes before publishing successful stdout.
pub(super) async fn run(
    executable: &str,
    args: &[String],
    signal: &Cancellation,
) -> io::Result<Vec<u8>> {
    if signal.is_aborted() {
        return Err(cancelled());
    }
    let mut child = Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("stderr unavailable"))?;
    let result = tokio::select! {
        result = async {
            let (status, stdout, ()) = tokio::try_join!(child.wait(), read_stdout(stdout), drain_stderr(stderr))?;
            if !status.success() {return Err(io::Error::other("search process failed"))}
            Ok(stdout)
        } => result,
        () = signal.cancelled() => Err(cancelled()),
    };
    if result.is_err() && child.try_wait()?.is_none() {
        child.kill().await?;
    }
    if signal.is_aborted() {
        return Err(cancelled());
    }
    result
}
/// Collect stdout bytes without interpreting chunk boundaries.
async fn read_stdout(mut pipe: impl AsyncRead + Unpin) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes).await?;
    Ok(bytes)
}
/// Consume stderr without retaining or displaying it.
async fn drain_stderr(mut pipe: impl AsyncRead + Unpin) -> io::Result<()> {
    tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await?;
    Ok(())
}
/// Cancellation represented as an interrupted host operation.
fn cancelled() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "search cancelled")
}
