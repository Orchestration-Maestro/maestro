//! Test support over the domain owners: silent command operations, the toolkit widgets a test
//! host supplies, the scripted provider a test configures, and the diagnostic writer it reports
//! through.

use crate::types::CommandContextOperations;

mod editor;
mod widgets;
/// The fallible standard error writer a test host reports bus and runner diagnostics through,
/// as the application's own bus and runner do.
pub use maestro_agent::diagnostics;
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
