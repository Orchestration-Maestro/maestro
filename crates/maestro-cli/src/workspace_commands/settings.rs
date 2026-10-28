//! Presentation of application-owned workspace settings operations.
use maestro_app::presentation_data::keybindings::reset_multiplexer_keybindings;
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
    if !args.is_empty() {
        writeln!(stderr, "usage: maestro settings reset-keys")?;
        return Ok(2);
    }
    let reset = match reset_multiplexer_keybindings(agent_dir, timestamp) {
        Ok(reset) => reset,
        Err(error) => {
            writeln!(
                stderr,
                "could not reset keybindings in {agent_dir}: {error}"
            )?;
            return Ok(1);
        }
    };
    writeln!(
        stdout,
        "Created backup: {}",
        reset
            .backup_path
            .as_ref()
            .map_or_else(String::new, |path| path.display().to_string())
    )?;
    writeln!(stdout, "Removed {} multiplexer overrides.", reset.removed)?;
    writeln!(
        stdout,
        "Run `maestro daemon reload` or restart the daemon to apply the defaults."
    )?;
    if let Some(error) = reset.dir_sync {
        writeln!(stderr, "Keybindings reset; directory sync warning: {error}")?;
    }
    Ok(0)
}

/// Present the application outcome without changing its commit status.
#[cfg(test)]
fn present_reset(
    _reset: maestro_app::presentation_data::keybindings::KeybindingsReset,
    _stdout: &mut dyn Write,
    _stderr: &mut dyn Write,
) -> io::Result<i32> {
    Err(io::Error::other("reset presentation unavailable"))
}

#[cfg(test)]
mod tests;
