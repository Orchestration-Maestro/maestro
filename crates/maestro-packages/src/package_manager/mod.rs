//! Configured-source policy over supplied effects and a live settings owner.
mod acquisition;
#[cfg(not(target_arch = "wasm32"))]
mod capture;
mod commands;
mod concurrency;
mod configuration;
mod git;
mod git_checks;
mod npm;
mod operations;
mod paths;
#[cfg(not(target_arch = "wasm32"))]
mod process;
mod progress;
mod sources;
mod update;
use maestro_settings::SettingsManager;
#[cfg(not(target_arch = "wasm32"))]
pub use operations::NativePackageOperations;
pub use operations::{CommandCaptureOptions, CommandOutput, PackageOperations};
pub use progress::{ProgressAction, ProgressCallback, ProgressEvent, ProgressEventType};
use std::{cell::RefCell, future::Future, io, pin::Pin, rc::Rc};
pub use update::{PackageUpdate, PackageUpdateType};

/// An object-safe fallible asynchronous operation.
pub type PackageFuture<'a, T> = Pin<Box<dyn Future<Output = io::Result<T>> + 'a>>;

/// The configuration scope selected for source lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstalledSourceScope {
    /// Sources configured in user settings.
    User,
    /// Sources configured in project settings.
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
/// Configured-source management, acquisition and removal over supplied effects.
pub struct DefaultPackageManager<O: PackageOperations> {
    /// Authored bases and live settings.
    options: PackageManagerOptions,
    /// Supplied effects.
    operations: O,
    /// The last successful command key and root, including empty results.
    root_cache: RefCell<(String, String)>,
    /// The callback used by the next progress emission.
    progress: RefCell<Option<ProgressCallback>>,
}
impl<O: PackageOperations> DefaultPackageManager<O> {
    /// Stores the supplied inputs without observing settings or effects.
    #[must_use]
    pub fn new(options: PackageManagerOptions, operations: O) -> Self {
        Self {
            options,
            operations,
            root_cache: RefCell::default(),
            progress: RefCell::default(),
        }
    }
}

/// Source acquisition, removal and configured-source operations.
pub trait PackageManager {
    /// Appends a source unless its identity exists in the selected scope; `None` means user.
    /// # Errors
    /// Returns consumed settings, borrow or path failures.
    fn add_source_to_settings(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool>;
    /// Removes every matching entry in the selected scope; `None` means user.
    /// # Errors
    /// Returns consumed settings, borrow or path failures.
    fn remove_source_from_settings(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool>;
    /// Lists original rows in user-then-project order with existing-content lookup.
    /// # Errors
    /// Returns consumed settings, borrow, path or command failures.
    fn list_configured_packages(&self) -> std::io::Result<Vec<ConfiguredPackage>>;
    /// Finds existing source contents without acquiring them.
    /// # Errors
    /// Returns required borrow, path or command failures.
    fn get_installed_path(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> std::io::Result<Option<String>>;
    /// Replaces or clears the progress callback; the next emission uses the replacement.
    fn set_progress_callback(&self, callback: Option<ProgressCallback>);
    /// Acquires or validates a source's contents; `None` means user scope.
    ///
    /// A failing start callback stops the operation; later failures, including the
    /// completion callback's, are reported as error events and returned, except that
    /// a failing error callback's failure is returned in place of the original.
    /// # Errors
    /// Returns callback, path, filesystem, settings or command failures.
    fn install<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Installs, then adds the source to settings; an existing entry is not an error.
    /// Nothing is flushed or rolled back.
    /// # Errors
    /// Returns installation failures or the settings edit's failure.
    fn install_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Removes a source's managed contents; `None` means user scope.
    /// # Errors
    /// Returns callback, path, filesystem or command failures.
    fn remove<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Removes the contents, then the settings entries; returns whether any entry was removed.
    /// Nothing is flushed or rolled back.
    /// # Errors
    /// Returns removal failures or the settings edit's failure.
    fn remove_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, bool>;
}
impl<O: PackageOperations> PackageManager for DefaultPackageManager<O> {
    fn add_source_to_settings(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool> {
        self.add_source(source, scope)
    }
    fn remove_source_from_settings(
        &self,
        source: &str,
        scope: Option<InstalledSourceScope>,
    ) -> std::io::Result<bool> {
        self.remove_source(source, scope)
    }
    fn list_configured_packages(&self) -> std::io::Result<Vec<ConfiguredPackage>> {
        self.configured_packages()
    }
    fn get_installed_path(
        &self,
        source: &str,
        scope: InstalledSourceScope,
    ) -> std::io::Result<Option<String>> {
        self.installed_path(source, scope)
    }
    fn set_progress_callback(&self, callback: Option<ProgressCallback>) {
        self.replace_progress_callback(callback);
    }
    fn install<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()> {
        Box::pin(self.acquire(source, scope))
    }
    fn install_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()> {
        Box::pin(self.acquire_and_persist(source, scope))
    }
    fn remove<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()> {
        Box::pin(self.release(source, scope))
    }
    fn remove_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, bool> {
        Box::pin(self.release_and_persist(source, scope))
    }
}
#[cfg(test)]
mod tests;
