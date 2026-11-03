//! Shared command operations for controlled contexts.

use crate::types::CommandContextOperations;

/// Command operations that answer plainly without touching a session.
#[must_use]
pub fn silent_command_operations() -> CommandContextOperations {
    CommandContextOperations::unbound()
}
