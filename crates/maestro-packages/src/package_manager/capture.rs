//! Native stdout capture through child exit, pipe EOFs and an optional deadline.
use super::{CommandCaptureOptions, NativePackageOperations};
use maestro_process::child_process::{
    CapturedChild, Termination, WaitOptions, wait_for_child_process,
};
use std::{cell::Cell, io, process::ExitStatus};

impl NativePackageOperations {
    /// Captures both streams without involving stdout takeover.
    pub(super) async fn capture_async(
        &self,
        command: &str,
        args: &[String],
        options: CommandCaptureOptions<'_>,
    ) -> io::Result<String> {
        let mut builder = tokio::process::Command::from(Self::command(command, args, options.env)?);
        if let Some(cwd) = options.cwd {
            builder.current_dir(cwd);
        }
        let mut child = CapturedChild::spawn(&mut builder)?;
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
            return Ok(crate::trim(&out).to_owned());
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
    #[cfg(unix)]
    {
        use rustix::process::Signal;
        use std::os::unix::process::ExitStatusExt;
        let names = [
            (Signal::HUP, "SIGHUP"),
            (Signal::INT, "SIGINT"),
            (Signal::QUIT, "SIGQUIT"),
            (Signal::ILL, "SIGILL"),
            (Signal::TRAP, "SIGTRAP"),
            (Signal::ABORT, "SIGABRT"),
            (Signal::BUS, "SIGBUS"),
            (Signal::FPE, "SIGFPE"),
            (Signal::KILL, "SIGKILL"),
            (Signal::USR1, "SIGUSR1"),
            (Signal::SEGV, "SIGSEGV"),
            (Signal::USR2, "SIGUSR2"),
            (Signal::PIPE, "SIGPIPE"),
            (Signal::ALARM, "SIGALRM"),
            (Signal::TERM, "SIGTERM"),
            (Signal::CHILD, "SIGCHLD"),
            (Signal::CONT, "SIGCONT"),
            (Signal::STOP, "SIGSTOP"),
            (Signal::TSTP, "SIGTSTP"),
            (Signal::TTIN, "SIGTTIN"),
            (Signal::TTOU, "SIGTTOU"),
            (Signal::URG, "SIGURG"),
            (Signal::XCPU, "SIGXCPU"),
            (Signal::XFSZ, "SIGXFSZ"),
            (Signal::VTALARM, "SIGVTALRM"),
            (Signal::PROF, "SIGPROF"),
            (Signal::WINCH, "SIGWINCH"),
            (Signal::SYS, "SIGSYS"),
            (Signal::IO, "SIGIO"),
            #[cfg(target_os = "linux")]
            (Signal::STKFLT, "SIGSTKFLT"),
            #[cfg(target_os = "linux")]
            (Signal::POWER, "SIGPWR"),
            #[cfg(target_os = "macos")]
            (Signal::EMT, "SIGEMT"),
            #[cfg(target_os = "macos")]
            (Signal::INFO, "SIGINFO"),
        ];
        if let Some((_, name)) = names
            .into_iter()
            .find(|(signal, _)| Some(signal.as_raw()) == status.signal())
        {
            return format!("signal {name}");
        }
    }
    "signal unknown".into()
}
