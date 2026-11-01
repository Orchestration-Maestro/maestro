//! A configured-value fixture for both native and browser test targets.
use maestro_app::{ConfigValueOperations, ConfigValues, SharedHandle};

/// Commands that never produce a value, as a failing shell command does.
pub struct FailingCommands;

impl ConfigValueOperations for FailingCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Configured-value commands that return command failure without spawning a process.
#[must_use]
pub fn no_commands() -> ConfigValues {
    SharedHandle::new(FailingCommands)
}
