//! Native stdout capture through child exit, pipe EOFs and an optional deadline.
use super::{CommandCaptureOptions, NativePackageOperations};
use std::{
    io,
    process::{ExitStatus, Stdio},
};
use tokio::io::{AsyncRead, AsyncReadExt};

impl NativePackageOperations {
    /// Captures both streams without involving stdout takeover.
    pub(super) async fn capture_async(
        &self,
        command: &str,
        args: &[String],
        options: CommandCaptureOptions<'_>,
    ) -> io::Result<String> {
        let mut builder = tokio::process::Command::from(self.command(command, args));
        if let Some(cwd) = options.cwd {
            builder.current_dir(cwd);
        }
        builder
            .envs(options.env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = builder.spawn()?;
        let stdout = read_stream(child.stdout.take());
        let stderr = read_stream(child.stderr.take());
        let timeout = options.timeout.map(tokio::time::sleep);
        let timer = async {
            if let Some(timeout) = timeout {
                timeout.await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(stdout, stderr, timer);
        let (mut out, mut err, mut status) = (None, None, None);
        let mut timed_out = false;
        while out.is_none() || err.is_none() || status.is_none() {
            tokio::select! {
                value = &mut stdout, if out.is_none() => out = Some(value?),
                value = &mut stderr, if err.is_none() => err = Some(value?),
                value = child.wait(), if status.is_none() => status = Some(value?),
                () = &mut timer, if !timed_out => {
                    timed_out = true;
                    terminate(&mut child);
                }
            }
        }
        let prefix = format!("{command} {}", args.join(" "));
        if timed_out {
            return Err(io::Error::other(format!(
                "{prefix} timed out after {}ms",
                options.timeout.map_or(0, |value| value.as_millis())
            )));
        }
        let status = status.ok_or_else(|| io::Error::other("child status unavailable"))?;
        let out = String::from_utf8_lossy(out.as_deref().unwrap_or_default());
        let err = String::from_utf8_lossy(err.as_deref().unwrap_or_default());
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
/// Accumulates bytes before replacement-character decoding at the result boundary.
async fn read_stream(stream: Option<impl AsyncRead + Unpin>) -> io::Result<Vec<u8>> {
    let mut stream = stream.ok_or_else(|| io::Error::other("child pipe unavailable"))?;
    let mut bytes = Vec::new();
    stream.read_to_end(&mut bytes).await?;
    Ok(bytes)
}
/// Requests ordinary platform termination once, ignoring an already-exited child.
fn terminate(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    if let Some(pid) = child
        .id()
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw)
    {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
    }
    #[cfg(not(unix))]
    let _ = child.start_kill();
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
