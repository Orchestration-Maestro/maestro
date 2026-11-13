//! A configured-value fixture for both native and browser test targets.
use maestro_models::SharedHandle;
use maestro_models_registry::ConfigValues;

/// Configured-value commands that return command failure without spawning a process.
#[must_use]
pub fn no_commands() -> ConfigValues {
    SharedHandle::new(maestro_credentials::test_support::NoCommands)
}
