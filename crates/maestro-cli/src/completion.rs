//! Effect-free shell completion output from the shared command metadata.

use std::io::{self, Write};

/// Shells supported by the command-line completion generator.
pub const SUPPORTED_SHELLS: [&str; 5] = ["bash", "elvish", "fish", "powershell", "zsh"];

/// Generate completion output for the one shell named after `completion`.
///
/// # Errors
/// Returns output failures unchanged.
pub fn run_completion(
    _args: &[String],
    _stdout: &mut dyn Write,
    _stderr: &mut dyn Write,
) -> io::Result<i32> {
    Ok(2)
}
