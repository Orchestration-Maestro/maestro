//! Authored path operations delegated to the installed native host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionAPI, ExtensionResult};

/// Creates a distinct directory under the host's temporary root, private on Unix.
///
/// The prefix must be a nonempty leaf name. The caller owns cleanup.
///
/// # Errors
/// Returns an invalid prefix, native creation failure or invalid response.
pub fn make_temp_dir(api: &ExtensionAPI, prefix: &str) -> ExtensionResult<String> {
    api.helper("paths.make_temp_dir", &(prefix,))
}

/// Returns the final path component using the host's path flavor.
///
/// # Errors
/// Returns a host helper failure or an invalid response.
pub fn basename(api: &ExtensionAPI, path: &str) -> ExtensionResult<String> {
    api.helper("paths.basename", &(path,))
}

/// Expands the authored path using the host home, never the component environment.
///
/// # Errors
/// Returns the native path owner's failure or an invalid response.
pub fn expand_path(api: &ExtensionAPI, file_path: &str) -> ExtensionResult<String> {
    api.helper("paths.expand_path", &(file_path,))
}
/// Spells a native host path the way this component's standard library opens it.
///
/// Host paths, such as the context's cwd or a [`resolve`] result, use the native
/// spelling. Convert one just before a `std::fs` call and keep the native text
/// for display. Windows drive and share paths change; Unix paths do not.
///
/// # Errors
/// Returns a host helper failure or an invalid response.
pub fn guest_path(api: &ExtensionAPI, path: &str) -> ExtensionResult<String> {
    api.helper("paths.guest_path", &(path,))
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
