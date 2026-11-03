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
        let mut child = config(executable, profile, args)?
            .launch()
            .map_err(|error| {
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

/// The Blink settings that describe a hover-capable desktop pointer to a headless Chromium.
const DESKTOP_POINTER_SETTINGS: &str =
    "primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4";

/// The driver's own launch switches, in the key and value form the library joins for Chromium.
fn own_switches() -> [(&'static str, Option<&'static str>); 4] {
    [
        ("force-device-scale-factor", Some("1.25")),
        ("js-flags", Some("--expose-gc")),
        ("blink-settings", Some(DESKTOP_POINTER_SETTINGS)),
        ("no-startup-window", None),
    ]
}

/// Build the launch configuration from the driver's switches and the caller's.
fn config(executable: &Path, profile: &Path, args: &[String]) -> io::Result<BrowserConfig> {
    let builder = own_switches().into_iter().fold(
        BrowserConfig::builder()
            .chrome_executable(executable)
            .user_data_dir(profile)
            .no_sandbox(),
        |builder, (key, value)| match value {
            Some(value) => builder.arg((key, value)),
            None => builder.arg(key),
        },
    );
    let builder = args.iter().try_fold(builder, |builder, argument| {
        let (key, value) = switch(argument)?;
        Ok::<_, io::Error>(match value {
            Some(value) => builder.arg((key, value)),
            None => builder.arg(key),
        })
    })?;
    builder.build().map_err(io::Error::other)
}

/// Split a caller's `--key[=value]` switch into the key and value the library joins itself.
fn switch(argument: &str) -> io::Result<(&str, Option<&str>)> {
    let body = argument
        .strip_prefix("--")
        .filter(|body| !body.is_empty() && !body.starts_with(['-', '=']))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("browser switch must be --key or --key=value: {argument}"),
            )
        })?;
    Ok(body
        .split_once('=')
        .map_or((body, None), |(key, value)| (key, Some(value))))
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
