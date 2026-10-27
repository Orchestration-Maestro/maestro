//! Authored paths expanded and resolved by the installed native host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult};
/// Expands the authored path using the host home, never the component environment.
///
/// # Errors
/// Returns the native path owner's failure or an invalid response.
pub fn expand_path(api: &ExtensionAPI, file_path: &str) -> ExtensionResult<String> {
    api.helper("paths.expand_path", &(file_path,))
}
/// Resolves path operands lexically with the host platform's path rules.
///
/// Keeps Unicode spaces and literal `~` and `@` prefixes without filesystem lookup.
///
/// # Errors
/// Returns the native path owner's failure or an invalid response.
pub fn resolve(api: &ExtensionAPI, paths: &[&str]) -> ExtensionResult<String> {
    api.helper("paths.resolve", &(paths,))
}

/// Resolves an authored path against the supplied host directory.
///
/// # Errors
/// Returns the native path owner's failure or an invalid response.
pub fn resolve_to_cwd(api: &ExtensionAPI, file_path: &str, cwd: &str) -> ExtensionResult<String> {
    api.helper("paths.resolve_to_cwd", &(file_path, cwd))
}

/// Finds the existing native spelling using the host filesystem and fallback order.
///
/// # Errors
/// Returns the native path owner's failure or an invalid response.
pub fn resolve_read_path(
    api: &ExtensionAPI,
    file_path: &str,
    cwd: &str,
) -> ExtensionResult<String> {
    api.helper("paths.resolve_read_path", &(file_path, cwd))
}
