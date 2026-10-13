//! Platform process signalling and fire-and-forget tree termination.
use std::io;

/// Sends a signal to a valid positive native PID or its group.
#[cfg(unix)]
pub(super) fn signal(pid: u32, signal: rustix::process::Signal, group: bool) -> io::Result<()> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid process ID"))?;
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
    use std::process::{Command, Stdio};
    let mut command = Command::new("taskkill");
    command
        .args(["/F", "/T", "/PID", &pid.to_string()])
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
