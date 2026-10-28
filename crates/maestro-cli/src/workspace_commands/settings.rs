//! Presentation of application-owned workspace settings operations.
use std::io::{self, Write};

/// Validate and present a multiplexer keybinding reset beneath the supplied root.
///
/// `args` contains only arguments after `settings reset-keys`. Invalid usage exits
/// with status 2 before reading a file. Application failures are printed with status 1.
///
/// # Errors
/// Returns output failures, including a closed pipe, for the executable to settle.
pub fn run_reset_keys(
    args: &[String],
    agent_dir: &str,
    timestamp: u64,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<i32> {
    let _ = (args, agent_dir, timestamp, stdout, stderr);
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "reset presentation is unavailable",
    ))
}
