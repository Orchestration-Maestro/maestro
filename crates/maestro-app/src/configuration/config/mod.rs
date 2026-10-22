//! Installation identity and paths.

pub mod installation;
pub mod paths;

#[cfg(not(target_arch = "wasm32"))]
mod self_update;
#[cfg(not(target_arch = "wasm32"))]
pub use self_update::{
    NativeSelfUpdateOperations, SelfUpdateCommand, SelfUpdateCommandStep, SelfUpdateContext,
    SelfUpdateOperations, get_self_update_command, get_self_update_unavailable_instruction,
    get_update_instruction,
};

use std::sync::LazyLock;

/// The distribution package name.
pub const PACKAGE_NAME: &str = "maestro";
/// The application command name.
pub const APP_NAME: &str = "maestro";
/// The application display title.
pub const APP_TITLE: &str = "Maestro";
pub use maestro_resources::paths::CONFIG_DIR_NAME;
/// The compiled application package version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The service origin, empty until configured.
pub const SERVICE_ORIGIN: &str = "";
/// The application-derived agent-directory environment key.
pub static ENV_AGENT_DIR: LazyLock<String> =
    LazyLock::new(|| format!("{}_CODING_AGENT_DIR", APP_NAME.to_uppercase()));
/// The application-derived session-directory environment key.
pub static ENV_SESSION_DIR: LazyLock<String> =
    LazyLock::new(|| format!("{}_CODING_AGENT_SESSION_DIR", APP_NAME.to_uppercase()));

/// Read the native agent-directory override, obtaining home only when needed.
///
/// # Errors
/// Returns the native home lookup error when the chosen path requires home.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_agent_dir() -> std::io::Result<String> {
    let value = std::env::var(&*ENV_AGENT_DIR).ok();
    paths::get_agent_dir(value.as_deref(), || maestro_tools::expand_path("~"))
}

/// Join an enabled hosted-service origin with its literal path.
pub(super) fn service_endpoint(service_origin: &str, path: &str) -> Option<String> {
    (!service_origin.is_empty()).then(|| format!("{service_origin}{path}"))
}

/// Build a literal viewer fragment when the supplied hosted origin is enabled.
#[must_use]
pub fn get_share_viewer_url(gist_id: &str, service_origin: &str) -> Option<String> {
    service_endpoint(service_origin, "/session/").map(|url| format!("{url}#{gist_id}"))
}
