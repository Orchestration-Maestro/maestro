//! Linux terminal creation with a caller-supplied setup entry.
use super::super::{Pty, PtyChild, PtyCommand, PtyEnvironment, PtySize, fd};
use std::{
    convert::Infallible,
    fs::File,
    io::{self, Read, Write},
    process::{Command, Stdio},
};

/// Opens a terminal and runs the caller's setup entry on its slave.
///
/// The adapter replaces executable/arguments and preserves directory/environment policy.
/// New terminals start with zero pixel totals.
///
/// # Errors
/// Returns native launch failures or the original setup/exec failure text.
pub fn spawn(
    size: PtySize,
    command: PtyCommand,
    setup: impl FnOnce(PtyCommand) -> io::Result<PtyCommand>,
) -> io::Result<(Pty, PtyChild)> {
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)?;
    grantpt(&master)?;
    unlockpt(&master)?;
    fd::resize_pty_fd(
        &master,
        PtySize {
            cell_width_px: 0,
            cell_height_px: 0,
            ..size
        },
    )?;
    let name = ptsname(&master, Vec::new())?;
    let slave = rustix::fs::open(
        name.as_c_str(),
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    let mut command = native_command(setup(command)?);
    let (mut error_stream, error_writer) = io::pipe()?;
    command
        .stdout(Stdio::from(fd::duplicate(&slave)?))
        .stdin(Stdio::from(slave))
        .stderr(Stdio::from(error_writer));
    let mut child = command.spawn()?;
    drop(command);
    let mut error_text = String::new();
    let read = error_stream.read_to_string(&mut error_text);
    if let Err(error) = read {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    if !error_text.is_empty() {
        let _ = child.wait();
        return Err(io::Error::other(error_text));
    }
    Ok((Pty { master_fd: master }, child))
}

/// Builds the native command once from its owned launch policy.
fn native_command(description: PtyCommand) -> Command {
    let mut command = crate::command(description.program).into_std();
    if let Some(argv0) = description.argv0 {
        use std::os::unix::process::CommandExt;
        command.arg0(argv0);
    }
    command.args(description.args);
    if let Some(cwd) = description.cwd {
        command.current_dir(cwd);
    }
    match description.environment {
        PtyEnvironment::Inherit(edits) => {
            for (key, value) in edits {
                if let Some(value) = value {
                    command.env(key, value);
                } else {
                    command.env_remove(key);
                }
            }
        }
        PtyEnvironment::Replace(values) => {
            command.env_clear().envs(values);
        }
    }
    command
}

/// Makes this process the terminal session leader and replaces it with the target.
///
/// Returns only on failure, reporting that same error through the retained setup stream.
///
/// # Errors
/// The returned error is the original setup or target-exec failure.
#[must_use]
pub fn setup_and_exec(command: Command) -> io::Error {
    let error_stream = match fd::duplicate(io::stderr()) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = io::stderr().write_all(error.to_string().as_bytes());
            return error;
        }
    };
    let error = match exec_target(command) {
        Err(error) => error,
        Ok(never) => match never {},
    };
    let _ = File::from(error_stream).write_all(error.to_string().as_bytes());
    error
}

/// Installs signal reset before creating a session and acquiring the slave terminal.
fn exec_target(command: Command) -> io::Result<Infallible> {
    use process_wrap::tokio::{CommandWrap, CommandWrapper, ResetSigmask};
    use std::os::unix::process::CommandExt;
    use tokio::signal::unix::{SignalKind, signal};
    let runtime = crate::child_process::runtime()?;
    let _entered = runtime.enter();
    let _signals = [
        SignalKind::hangup(),
        SignalKind::interrupt(),
        SignalKind::quit(),
        SignalKind::alarm(),
        SignalKind::terminate(),
        SignalKind::child(),
    ]
    .into_iter()
    .map(signal)
    .collect::<io::Result<Vec<_>>>()?;
    let mut command = tokio::process::Command::from(command);
    let context = CommandWrap::from(tokio::process::Command::new(""));
    ResetSigmask.pre_spawn(&mut command, &context)?;
    rustix::process::setsid()?;
    rustix::process::ioctl_tiocsctty(io::stdin())?;
    command.stdin(Stdio::from(fd::duplicate(io::stdin())?));
    command.stdout(Stdio::from(fd::duplicate(io::stdin())?));
    command.stderr(Stdio::from(fd::duplicate(io::stdin())?));
    Err(command.into_std().exec())
}

#[cfg(test)]
mod tests;
