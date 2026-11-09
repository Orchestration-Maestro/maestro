//! Test support over the domain owners: silent command operations, the toolkit widgets a test
//! host supplies, and the scripted provider a test configures.

use crate::types::CommandContextOperations;

mod editor;
mod widgets;
pub use maestro_models::{
    FauxModelDefinition, FauxProviderRegistration, FauxResponseStep, FauxToolCallOptions,
    RegisterFauxProviderOptions, faux_text, faux_thinking, faux_tool_call, register_faux_provider,
};
pub use widgets::ToolkitWidgets;

/// Command operations that answer plainly without touching a session.
#[must_use]
pub fn silent_command_operations() -> CommandContextOperations {
    CommandContextOperations::unbound()
}
