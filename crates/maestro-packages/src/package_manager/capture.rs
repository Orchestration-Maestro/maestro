//! Native stdout capture through child exit, pipe EOFs and an optional deadline.
use super::{CommandCaptureOptions, NativePackageOperations};
use maestro_process::child_process::{Termination, WaitOptions, wait_for_child_process};
use std::{cell::Cell, io, process::ExitStatus};

impl NativePackageOperations {
    /// Captures both streams without involving stdout takeover.
    pub(super) async fn capture_async(
        &self,
        command: &str,
        args: &[String],
        options: CommandCaptureOptions<'_>,
    ) -> io::Result<String> {
        let mut child = Self::command(command, args, options.env, options.cwd).spawn_captured()?;
        let timed_out = Cell::new(false);
        let deadline = async {
            if let Some(timeout) = options.timeout {
                tokio::time::sleep(timeout).await;
            } else {
                std::future::pending::<()>().await;
            }
            timed_out.set(true);
            Termination::Terminate
        };
        let output =
            wait_for_child_process(&mut child, WaitOptions::default(), deadline, None).await?;
        let prefix = format!("{command} {}", args.join(" "));
        if timed_out.get() {
            return Err(io::Error::other(format!(
                "{prefix} timed out after {}ms",
                options.timeout.map_or(0, |value| value.as_millis())
            )));
        }
        let status = output.status;
        let out = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        if status.success() {
            return Ok(if command == "git" && args.iter().any(|arg| arg == "-z") {
                out.into_owned()
            } else {
                crate::trim(&out).to_owned()
            });
        }
        let payload = if err.is_empty() { &out } else { &err };
        Err(io::Error::other(format!(
            "{prefix} failed with {}: {payload}",
            exit_status(status)
        )))
    }
}
/// Renders an exit code, or the supported native signal name.
fn exit_status(status: ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("code {code}");
    }
    maestro_process::exit_signal_name(status)
        .map_or_else(|| "signal unknown".into(), |name| format!("signal {name}"))
}
