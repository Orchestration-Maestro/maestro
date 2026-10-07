use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Compile with a supplied command, logging diagnostics on failure.
///
/// # Errors
/// Returns native process and filesystem failures.
pub(crate) fn run(child: &mut Command, log: &Path) -> io::Result<ExitCode> {
    let output = child.output()?;
    if output.status.success() {
        return Ok(ExitCode::SUCCESS);
    }
    fs::write(log, output.stderr)?;
    eprintln!("Browser smoke check failed. See {}", log.display());
    Ok(ExitCode::FAILURE)
}
