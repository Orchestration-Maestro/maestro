//! Captures native observations without replacing invalid filesystem bytes.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    child_process::{ChildProcess, SpawnOptions, Stdio},
    streams::{Readable, read_to_end},
};
use std::{
    future::{Future, poll_fn},
    pin::pin,
    rc::Rc,
    task::Poll,
};

/// Retains cancellation ownership while a native observation is pending.
struct Observation {
    /// Original native controller, including its existing reaper.
    child: Rc<dyn ChildProcess>,
}
impl Drop for Observation {
    fn drop(&mut self) {
        if let Err(error) = self.child.kill(Some("SIGKILL".into())) {
            eprintln!("Cannot stop sandbox observation: {error}");
        }
    }
}

/// Captures stdout as bytes while draining stderr independently.
pub(crate) async fn bytes(
    api: &ExtensionAPI,
    program: &str,
    args: &[String],
    codes: &[i32],
) -> ExtensionResult<Vec<u8>> {
    let io = api.native_io();
    let child = Observation {
        child: io
            .spawn(
                program.into(),
                args.to_vec(),
                SpawnOptions {
                    stdio: [Stdio::Ignore, Stdio::Pipe, Stdio::Pipe],
                    ..SpawnOptions::default()
                },
            )
            .await
            .map_err(|error| format!("Sandbox prerequisite {program}: {error}"))?,
    };
    let output = child
        .child
        .stdout()
        .ok_or("Sandbox observation stdout is unavailable")?;
    let error = child
        .child
        .stderr()
        .ok_or("Sandbox observation stderr is unavailable")?;
    let (output, error) = read_pair(output, error).await?;
    let status = child.child.wait().await?;
    if status.code.is_none_or(|code| !codes.contains(&code)) {
        return Err(format!(
            "Sandbox prerequisite {program} failed ({status:?}): {}",
            String::from_utf8_lossy(&error)
        ));
    }
    Ok(output)
}

/// Rejects unrepresentable observed paths rather than selecting a different native file.
pub(crate) async fn command_output(
    api: &ExtensionAPI,
    program: &str,
    args: &[String],
) -> ExtensionResult<String> {
    String::from_utf8(bytes(api, program, args, &[0]).await?)
        .map_err(|error| format!("Sandbox observation is not UTF-8: {error}"))
}

/// Settles both pipes without a sequential-read deadlock.
async fn read_pair(
    output: Rc<dyn Readable>,
    error: Rc<dyn Readable>,
) -> ExtensionResult<(Vec<u8>, Vec<u8>)> {
    let mut first = pin!(read_to_end(output));
    let mut second = pin!(read_to_end(error));
    let mut output = None;
    let mut error = None;
    poll_fn(|cx| {
        if output.is_none()
            && let Poll::Ready(result) = first.as_mut().poll(cx)
        {
            output = Some(result?);
        }
        if error.is_none()
            && let Poll::Ready(result) = second.as_mut().poll(cx)
        {
            error = Some(result?);
        }
        match (output.take(), error.take()) {
            (Some(output), Some(error)) => Poll::Ready(Ok((output, error))),
            (left, right) => {
                output = left;
                error = right;
                Poll::Pending
            }
        }
    })
    .await
}
