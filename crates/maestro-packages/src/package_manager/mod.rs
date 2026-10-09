//! Configured-source policy over supplied effects and a live settings owner.
mod commands;
mod configuration;
mod operations;
mod paths;
mod sources;
use maestro_settings::SettingsManager;
#[cfg(not(target_arch = "wasm32"))]
pub use operations::NativePackageOperations;
pub use operations::{CommandOutput, PackageOperations};
use std::{cell::RefCell, rc::Rc};

/// The configuration scope containing installed contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstalledSourceScope {
    /// Contents under the supplied agent directory.
    User,
    /// Contents under the project configuration directory.
    Project,
}
/// One configured source in its original list position.
#[derive(Debug, PartialEq, Eq)]
pub struct ConfiguredPackage {
    /// The stored source spelling.
    pub source: String,
    /// The source's configuration scope.
    pub scope: InstalledSourceScope,
    /// Whether the stored entry is an object.
    pub filtered: bool,
    /// Existing contents, when found.
    pub installed_path: Option<String>,
}
/// Authored bases and the shared settings owner.
pub struct PackageManagerOptions {
    /// The input source base.
    pub cwd: String,
    /// The user configuration base.
    pub agent_dir: String,
    /// The same settings owner retained by callers.
    pub settings_manager: Rc<RefCell<SettingsManager>>,
}
/// Configured-source management without acquisition.
pub struct DefaultPackageManager<O: PackageOperations> {
    /// Authored bases and live settings.
    options: PackageManagerOptions,
    /// Supplied effects.
    operations: O,
    /// The last successful command key and root, including empty results.
    root_cache: (String, String),
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Stores the supplied inputs without observing settings or effects.
    #[must_use]
    pub fn new(options: PackageManagerOptions, operations: O) -> Self {
        Self {
            options,
            operations,
            root_cache: (String::new(), String::new()),
        }
    }
}

/// Configured-source operations, with no acquisition or update methods.
pub trait PackageManager {
    /// Appends a source unless its identity exists in the selected scope; `None` means user.
    /// # Errors
    /// Returns consumed settings, borrow or path failures.
    fn add_source_to_settings(
        &mut self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool>;
    /// Removes every matching entry in the selected scope; `None` means user.
    /// # Errors
    /// Returns consumed settings, borrow or path failures.
    fn remove_source_from_settings(
        &mut self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool>;
    /// Lists original rows in user-then-project order with existing-content lookup.
    /// # Errors
    /// Returns consumed settings, borrow, path or command failures.
    fn list_configured_packages(&mut self) -> std::io::Result<Vec<ConfiguredPackage>>;
    /// Finds existing source contents without acquiring them.
    /// # Errors
    /// Returns required borrow, path or command failures.
    fn get_installed_path(
        &mut self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> std::io::Result<Option<String>>;
}
impl<O: PackageOperations> PackageManager for DefaultPackageManager<O> {
    fn add_source_to_settings(
        &mut self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool> {
        self.add_source(source, scope)
    }
    fn remove_source_from_settings(
        &mut self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool> {
        self.remove_source(source, scope)
    }
    fn list_configured_packages(&mut self) -> std::io::Result<Vec<ConfiguredPackage>> {
        self.configured_packages()
    }
    fn get_installed_path(
        &mut self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> std::io::Result<Option<String>> {
        self.installed_path(source, scope)
    }
}
#[cfg(test)]
mod tests;
