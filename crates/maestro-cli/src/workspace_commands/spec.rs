//! Effect-free command metadata shared by help and shell completion.

use clap::Command;

/// Describe the complete command-line surface without consuming chat input.
#[must_use]
pub fn command() -> Command {
    Command::new("maestro")
}
