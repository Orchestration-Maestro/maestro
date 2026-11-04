//! Commands that answer no value.

use crate::ConfigValueOperations;

/// Commands that never produce a value, as a failing shell command does.
pub struct NoCommands;

impl ConfigValueOperations for NoCommands {
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        None
    }
}
