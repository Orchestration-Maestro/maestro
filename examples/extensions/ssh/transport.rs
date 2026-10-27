//! Raw child pipes for SSH file observations and writes.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    child_process::{ChildProcess, SpawnOptions},
    streams::Readable,
};
use std::{rc::Rc, task::Poll};

/// Quotes one literal operand for a POSIX shell, including embedded apostrophes.
pub(super) fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// Drains original bytes without decoding or merging standard error into file contents.
async fn collect(stream: Rc<dyn Readable>) -> ExtensionResult<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.read().await? {
        bytes.extend(chunk);
    }
    Ok(bytes)
}

/// Polls all three pipes together so large writes and either output cannot deadlock.
async fn transfer(
    child: &Rc<dyn ChildProcess>,
    input: Option<String>,
) -> ExtensionResult<(Vec<u8>, Vec<u8>)> {
    let mut stdout = Box::pin(collect(child.stdout().ok_or("SSH stdout missing")?));
    let mut stderr = Box::pin(collect(child.stderr().ok_or("SSH stderr missing")?));
    let stdin = child.stdin().ok_or("SSH stdin missing")?;
    let mut write = Box::pin(async move {
        if let Some(input) = input {
            stdin.write(input.as_bytes()).await?;
        }
        stdin.end().await
    });
    let (mut output, mut error, mut written) = (None, None, false);
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
        if !written && let Poll::Ready(result) = write.as_mut().poll(cx) {
            result?;
            written = true;
        }
        if written
            && output.is_some()
            && error.is_some()
            && let (Some(output), Some(error)) = (output.take(), error.take())
        {
            return Poll::Ready(Ok((output, error)));
        }
        Poll::Pending
    })
    .await
}

/// Runs one SSH child, reaping it on pipe failure and retaining exit diagnostics.
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
    let (stdout, stderr) = match transfer(&child, input).await {
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
    Ok(stdout)
}
