//! Native child construction, synchronous capture and exit observation.
use super::{
    CommandOutput, PackageFuture,
    operations::{NativePackageOperations, recover_environment},
};
use std::{
    io,
    process::{Command, Stdio},
};

impl NativePackageOperations {
    /// Builds a child with literal arguments and the recovered environment; the native
    /// launcher resolves Windows command shims such as `npm.cmd`.
    ///
    /// # Errors
    /// Returns `NotFound` when a Windows command cannot be resolved.
    pub(super) fn command(command: &str, args: &[String]) -> io::Result<Command> {
        let mut child = maestro_process::Launch {
            executable: command.into(),
            args: args.into(),
            verbatim: false,
        }
        .command()?;
        let inherited = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        let environment = recover_environment(
            if cfg!(target_os = "linux") {
                "linux"
            } else {
                "other"
            },
            inherited,
            || std::fs::read("/proc/self/environ"),
        );
        child.env_clear().envs(environment);
        Ok(child)
    }

    /// Waits for a child with ignored stdin and decoded captured streams.
    pub(super) fn capture(command: &str, args: &[String]) -> io::Result<CommandOutput> {
        let output = Self::command(command, args)?
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;
        Ok(CommandOutput {
            status: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Runs a child with inherited streams, or with output routed to standard error while
    /// stdout is taken over, and settles when it exits rather than when its streams close.
    pub(super) fn launch<'a>(
        &'a self,
        command: &'a str,
        args: &'a [String],
        cwd: Option<&'a str>,
    ) -> PackageFuture<'a, Option<i32>> {
        Box::pin(async move {
            let mut child = tokio::process::Command::from(Self::command(command, args)?);
            if let Some(cwd) = cwd {
                child.current_dir(cwd);
            }
            if (self.is_stdout_taken_over)() {
                child
                    .stdin(Stdio::null())
                    .stdout(stderr_stdio()?)
                    .stderr(stderr_stdio()?);
            }
            Ok(child.spawn()?.wait().await?.code())
        })
    }
}

/// Duplicates this process's standard error as a child stream.
fn stderr_stdio() -> io::Result<Stdio> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        Ok(Stdio::from(io::stderr().as_fd().try_clone_to_owned()?))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsHandle;
        Ok(Stdio::from(io::stderr().as_handle().try_clone_to_owned()?))
    }
}
