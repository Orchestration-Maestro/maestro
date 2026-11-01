//! Native child construction, synchronous capture and exit observation.
use super::{
    CommandOutput, PackageFuture,
    operations::{NativePackageOperations, recover_environment},
};
use std::{io, process::Stdio};

impl NativePackageOperations {
    /// Collects package operands, the recovered environment and child directory for launch.
    pub(super) fn command<'a>(
        command: &str,
        args: &[String],
        overrides: &[(&str, &str)],
        cwd: Option<&'a str>,
    ) -> maestro_process::LaunchContext<'a, Vec<(String, String)>> {
        let inherited = maestro_process::environment()
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
        let environment: Vec<_> = environment
            .into_iter()
            .chain(
                overrides
                    .iter()
                    .map(|&(key, value)| (key.into(), value.into())),
            )
            .collect();
        maestro_process::Launch {
            executable: command.into(),
            args: args.into(),
            verbatim: false,
        }
        .with_context(environment, cwd.map(std::path::Path::new))
        .clear_environment()
    }

    /// Waits for a child with ignored stdin and decoded captured streams.
    pub(super) fn capture(command: &str, args: &[String]) -> io::Result<CommandOutput> {
        let output = Self::command(command, args, &[], None).output()?;
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
            let stdio = if (self.is_stdout_taken_over)() {
                [Stdio::null(), stderr_stdio()?, stderr_stdio()?]
            } else {
                [Stdio::inherit(), Stdio::inherit(), Stdio::inherit()]
            };
            Ok(Self::command(command, args, &[], cwd)
                .spawn(stdio)?
                .wait()
                .await?
                .code())
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
