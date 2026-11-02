//! Installation identity supplied by the native application host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult};
/// Reads the host installation version, not the component's build version.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn version(api: &ExtensionAPI) -> ExtensionResult<String> {
    api.helper("version", &())
}
/// Reads the agent directory selected by the application's configuration owner.
///
/// # Errors
/// Returns the host's failure or an invalid response.
pub fn get_agent_dir(api: &ExtensionAPI) -> ExtensionResult<String> {
    api.helper("config.get_agent_dir", &())
}
