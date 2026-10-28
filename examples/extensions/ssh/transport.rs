//! Raw child pipes for SSH file observations and writes.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    child_process::{ChildProcess, SpawnOptions},
    streams::Readable,
};
use std::{rc::Rc, task::Poll};

pub(super) use maestro_text::shell_quote as quote;

/// Drains original bytes without decoding or merging standard error into file contents.
async fn collect(stream: Rc<dyn Readable>) -> ExtensionResult<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.read().await? {
        bytes.extend(chunk);
    }
    Ok(bytes)
}

/// Drains both outputs beside input, retaining write errors for the exit-status check.
async fn transfer(
    child: &Rc<dyn ChildProcess>,
    input: Option<String>,
) -> ExtensionResult<(Vec<u8>, Vec<u8>, ExtensionResult<()>)> {
    let mut stdout = Box::pin(collect(child.stdout().ok_or("SSH stdout missing")?));
    let mut stderr = Box::pin(collect(child.stderr().ok_or("SSH stderr missing")?));
    let stdin = child.stdin().ok_or("SSH stdin missing")?;
    let mut write = Box::pin(async move {
        let result = if let Some(input) = input {
            stdin.write(input.as_bytes()).await
        } else {
            Ok(())
        };
        let ended = stdin.end().await;
        result.and(ended)
    });
    let (mut output, mut error, mut written) = (None, None, None);
    std::future::poll_fn(|cx| {
        if output.is_none()
            && let Poll::Ready(bytes) = stdout.as_mut().poll(cx)
        {
            output = Some(bytes?);
        }
        if error.is_none()
            && let Poll::Ready(bytes) = stderr.as_mut().poll(cx)
        {
            error = Some(bytes?);
        }
        if written.is_none()
            && let Poll::Ready(result) = write.as_mut().poll(cx)
        {
            written = Some(result);
        }
        if written.is_some()
            && output.is_some()
            && error.is_some()
            && let (Some(output), Some(error), Some(written)) =
                (output.take(), error.take(), written.take())
        {
            return Poll::Ready(Ok((output, error, written)));
        }
        Poll::Pending
    })
    .await
}

/// Runs one SSH child, reaping it on output-pipe failure and retaining exit diagnostics.
pub(super) async fn execute(
    api: &ExtensionAPI,
    remote: &str,
    command: String,
    input: Option<String>,
) -> ExtensionResult<Vec<u8>> {
    let io = api.native_io();
    let child = io
        .spawn(
            "ssh".into(),
            vec!["--".into(), remote.into(), command],
            SpawnOptions::default(),
        )
        .await?;
    let (stdout, stderr, written) = match transfer(&child, input).await {
        Ok(output) => output,
        Err(error) => {
            let _ = child.kill(None);
            let _ = child.wait().await;
            return Err(error);
        }
    };
    let status = child.wait().await?;
    if status.code != Some(0) {
        let code = status.code.map_or_else(
            || status.signal.unwrap_or_else(|| "unknown".into()),
            |code| code.to_string(),
        );
        return Err(format!(
            "SSH failed ({code}): {}",
            String::from_utf8_lossy(&stderr)
        ));
    }
    written?;
    Ok(stdout)
}
