//! Launch the explicitly supplied Chromium through the approved library.

use super::cleanup;
use chromiumoxide::{BrowserConfig, async_process::Child};
use std::{io, path::Path};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader, Lines};

/// Browser child plus the continuous stderr drain.
pub(super) struct Process {
    /// Library-owned native child, killed on drop by its existing launcher.
    child: Child,
    /// Drain retained until browser exit so diagnostics cannot block Chromium.
    diagnostics: tokio::task::JoinHandle<io::Result<()>>,
}

impl Process {
    /// Start with an exclusive profile and wait for the browser's readiness line.
    pub async fn launch(
        executable: &Path,
        profile: &Path,
        args: &[String],
    ) -> io::Result<(Self, String)> {
        let config = BrowserConfig::builder()
            .chrome_executable(executable)
            .user_data_dir(profile)
            .no_sandbox()
            .args(["--force-device-scale-factor=1.25", "--js-flags=--expose-gc"])
            .args(args.iter().cloned())
            .build()
            .map_err(io::Error::other)?;
        let mut child = config.launch().map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot launch {}: {error}", executable.display()),
            )
        })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Chromium stderr unavailable"))?;
        let mut lines = BufReader::new(stderr.into_inner()).lines();
        let endpoint = match ready(&mut lines).await {
            Ok(endpoint) => endpoint,
            Err(error) => {
                let stopped = if child.try_wait()?.is_some() {
                    Ok(())
                } else {
                    child.kill().await
                };
                return cleanup(Err(error), stopped);
            }
        };
        let diagnostics = tokio::spawn(async move {
            while let Some(line) = lines.next_line().await? {
                eprintln!("CHROMIUM {line}");
            }
            Ok(())
        });
        Ok((Self { child, diagnostics }, endpoint))
    }

    /// Wait for graceful browser completion before settling its diagnostic stream.
    pub async fn wait(mut self) -> io::Result<()> {
        let stopped = self.child.wait().await.map(|_| ());
        let drained = self.diagnostics.await.map_err(io::Error::other)?;
        stopped.and(drained)
    }

    /// Kill and reap after failure, then settle the stderr drain.
    pub async fn close(mut self) -> io::Result<()> {
        let stopped = if self.child.try_wait()?.is_some() {
            Ok(())
        } else {
            self.child.kill().await
        };
        let drained = self.diagnostics.await.map_err(io::Error::other)?;
        stopped.and(drained)
    }
}

/// Wait for Chromium's advertised endpoint or its diagnostic stream's EOF.
async fn ready(lines: &mut Lines<impl AsyncBufRead + Unpin>) -> io::Result<String> {
    let mut diagnostics = String::new();
    while let Some(line) = lines.next_line().await? {
        if let Some(url) = line.strip_prefix("DevTools listening on ") {
            return Ok(url.to_owned());
        }
        diagnostics.push_str(&line);
        diagnostics.push('\n');
    }
    Err(io::Error::other(format!(
        "chromium exited before readiness: {diagnostics}"
    )))
}

#[cfg(test)]
mod tests;
