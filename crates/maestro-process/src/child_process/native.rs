//! Platform process signalling and fire-and-forget tree termination.
use std::io;

/// Sends a signal to a valid positive native PID or its group.
#[cfg(unix)]
pub(super) fn signal(pid: u32, signal: rustix::process::Signal, group: bool) -> io::Result<()> {
    let pid = native_pid(pid)?;
    if group {
        rustix::process::kill_process_group(pid, signal)
    } else {
        rustix::process::kill_process(pid, signal)
    }
    .map_err(Into::into)
}

/// Force-kills the process group identified by a positive native PID.
///
/// # Errors
/// Returns an invalid-ID or native signal error.
#[cfg(unix)]
pub fn kill_process_group(pid: u32) -> io::Result<()> {
    signal(pid, rustix::process::Signal::KILL, true)
}

/// Force-kills the direct process identified by a positive native PID.
///
/// # Errors
/// Returns an invalid-ID or native signal error.
#[cfg(unix)]
pub fn kill_process(pid: u32) -> io::Result<()> {
    signal(pid, rustix::process::Signal::KILL, false)
}

/// Builds the native tree-kill operands with ignored standard streams.
#[cfg(any(windows, test))]
pub(super) fn tree_kill_command(pid: u32) -> std::process::Command {
    kill_command(pid, true)
}

/// Builds the existing native terminator for a direct process or its tree.
#[cfg(any(windows, test))]
fn kill_command(pid: u32, tree: bool) -> std::process::Command {
    use std::process::Stdio;
    let mut command = crate::command("taskkill").into_std();
    command.arg("/F");
    if tree {
        command.arg("/T");
    }
    command
        .args(["/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// Starts a detached taskkill request with ignored standard streams.
///
/// # Errors
/// Returns the original native spawn error.
#[cfg(windows)]
pub fn request_tree_kill(pid: u32) -> io::Result<()> {
    use std::os::windows::process::CommandExt;
    tree_kill_command(pid)
        .creation_flags(0x0000_0008)
        .spawn()
        .map(|_| ())
}

/// Native signal selected by a process-list caller.
#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signal {
    /// Hang up a terminal process.
    Hangup,
    /// Request ordinary termination.
    Terminate,
    /// Force immediate termination.
    Kill,
}

/// Sends one signal to valid positive PIDs, continuing after per-process failures.
#[cfg(unix)]
pub fn signal_processes(pids: &[u32], selected: Signal) {
    let selected = match selected {
        Signal::Hangup => rustix::process::Signal::HUP,
        Signal::Terminate => rustix::process::Signal::TERM,
        Signal::Kill => rustix::process::Signal::KILL,
    };
    for &pid in pids {
        let _ = signal(pid, selected, false);
    }
}

/// Treats permission denial as positive evidence of a live process.
#[cfg(unix)]
pub(super) fn exists_from_probe(result: rustix::io::Result<()>) -> bool {
    matches!(result, Ok(()) | Err(rustix::io::Errno::PERM))
}

/// Checks a valid positive native PID without sending a signal.
#[cfg(unix)]
#[must_use]
pub fn process_exists(pid: u32) -> bool {
    native_pid(pid).is_ok_and(|pid| exists_from_probe(rustix::process::test_kill_process(pid)))
}

/// Validates the positive PID domain shared by delivery and liveness probes.
#[cfg(unix)]
fn native_pid(pid: u32) -> io::Result<rustix::process::Pid> {
    i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid process ID"))
}

/// Delivers a named signal to a native PID or supported process group.
///
/// # Errors
/// Returns an invalid PID/name error or the original native delivery failure.
#[cfg(unix)]
pub fn signal_pid(pid: i32, name: Option<&str>) -> io::Result<()> {
    let name = name.unwrap_or("SIGTERM");
    let selected = super::exit_signal::NAMES
        .iter()
        .find(|(_, candidate)| *candidate == name)
        .map(|(signal, _)| *signal)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown signal {name}"),
            )
        })?;
    signal(pid.unsigned_abs(), selected, pid < 0)
}

/// Terminates only the direct Windows process with a supported termination signal.
///
/// # Errors
/// Returns an invalid PID/name or the native process-termination failure.
#[cfg(windows)]
pub fn signal_pid(pid: i32, name: Option<&str>) -> io::Result<()> {
    let name = name.unwrap_or("SIGTERM");
    if !matches!(name, "SIGTERM" | "SIGKILL" | "SIGINT" | "SIGQUIT") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Unknown signal {name}"),
        ));
    }
    let pid = u32::try_from(pid)
        .ok()
        .filter(|pid| *pid != 0)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "process groups are unavailable",
            )
        })?;
    let mut command = tokio::process::Command::from(kill_command(pid, false));
    let options = super::SpawnSyncOptions::Capture {
        stdout: super::OutputMode::Discard,
        stderr: super::OutputMode::Discard,
        input: None,
        timeout: None,
        max_buffer_bytes: None,
    };
    let output = super::spawn_sync(&mut command, options).map_err(io::Error::other)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "process termination exited with {}",
            output.status
        )))
    }
}

/// Reports that this native host has no PID signalling facility.
///
/// # Errors
/// Always returns the native unsupported-operation error.
#[cfg(not(any(unix, windows)))]
pub fn signal_pid(_pid: i32, _name: Option<&str>) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}
