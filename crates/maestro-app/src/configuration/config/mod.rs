//! Installation identity and paths.

pub mod installation;
pub mod paths;

#[cfg(not(target_arch = "wasm32"))]
mod self_update;
#[cfg(not(target_arch = "wasm32"))]
pub use self_update::{
    NativeSelfUpdateOperations, SelfUpdateCommand, SelfUpdateCommandStep, SelfUpdateContext,
    SelfUpdateOperations, SelfUpdatePlan, get_self_update_command,
    get_self_update_command_from_settings, get_self_update_plan,
    get_self_update_unavailable_instruction, get_self_update_unavailable_instruction_from_settings,
    get_update_instruction, run_self_update,
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

/// Select the package directory of the running process: the `MAESTRO_PACKAGE_DIR`
/// override first, with `~` expanded, then the directory beside a standalone executable,
/// or the root of the source workspace a build runs from.
///
/// # Errors
/// Returns the native home lookup error when the override needs home, or the error of
/// locating the running executable.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_package_dir() -> std::io::Result<String> {
    package_dir(
        installation::Installation::current,
        package_dir_override().as_deref(),
    )
}

/// The `MAESTRO_PACKAGE_DIR` override of the running process.
#[cfg(not(target_arch = "wasm32"))]
fn package_dir_override() -> Option<String> {
    std::env::var(format!("{}_PACKAGE_DIR", APP_NAME.to_uppercase())).ok()
}

/// Select the package directory from the override or the lazily observed installation.
#[cfg(not(target_arch = "wasm32"))]
fn package_dir(
    installation: impl FnOnce() -> std::io::Result<installation::Installation>,
    override_dir: Option<&str>,
) -> std::io::Result<String> {
    paths::get_package_dir(
        installation,
        override_dir,
        || maestro_tools::expand_path("~"),
        exists,
    )
}

/// Whether a native path exists.
#[cfg(not(target_arch = "wasm32"))]
fn exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

/// Resolve the README path of the running process's package directory.
///
/// # Errors
/// Returns package-directory or working-directory failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_readme_path() -> std::io::Result<String> {
    paths::get_readme_path(&get_package_dir()?)
}

/// Resolve the documentation directory of the running process's package directory.
///
/// # Errors
/// Returns package-directory or working-directory failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_docs_path() -> std::io::Result<String> {
    paths::get_docs_path(&get_package_dir()?)
}

/// Resolve the examples directory of the running process's package directory.
///
/// # Errors
/// Returns package-directory or working-directory failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_examples_path() -> std::io::Result<String> {
    paths::get_examples_path(&get_package_dir()?)
}

/// Select the shipped theme directory of the running process and the custom theme
/// directory of the environment-selected agent directory.
///
/// A run from a source checkout reads the shipped themes from the theme crate's assets
/// in the package directory; a standalone executable reads them from the `theme` folder
/// in the package directory. The running executable is located only when no package
/// override applies.
///
/// # Errors
/// Returns package-directory or agent-directory failures.
#[cfg(not(target_arch = "wasm32"))]
pub fn get_theme_directories() -> std::io::Result<maestro_theme::ThemeDirectories> {
    theme_directories(package_dir_override().as_deref(), std::env::current_exe)
}

/// Select the theme directories from the override or the installation `executable`
/// completes, probing the layout once.
#[cfg(not(target_arch = "wasm32"))]
fn theme_directories(
    override_dir: Option<&str>,
    executable: impl FnOnce() -> std::io::Result<std::path::PathBuf>,
) -> std::io::Result<maestro_theme::ThemeDirectories> {
    let (module_dir, layout) = installation::probe_layout();
    let package_dir = package_dir(
        || installation::Installation::locate(module_dir, layout, executable),
        override_dir,
    )?;
    Ok(maestro_theme::ThemeDirectories {
        themes_dir: paths::get_themes_dir(&package_dir, layout),
        custom_themes_dir: paths::get_custom_themes_dir(&get_agent_dir()?),
    })
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

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;
