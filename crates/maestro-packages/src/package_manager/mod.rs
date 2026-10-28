//! Configured-source policy over supplied effects and a live settings owner.
mod acquisition;
mod automatic_resources;
#[cfg(not(target_arch = "wasm32"))]
mod capture;
mod commands;
mod concurrency;
mod configuration;
mod configured_resolution;
mod git;
mod git_checks;
mod inspection;
mod integrity;
pub use inspection::GitRelease;
pub use integrity::GitPinStatus;
mod glob;
mod manifest;
mod npm;
mod operations;
mod package_filters;
mod paths;
#[cfg(not(target_arch = "wasm32"))]
mod process;
mod progress;
mod resolution;
mod resource_discovery;
mod resource_patterns;
mod sources;
use maestro_resources::SourceScope;
pub use resolution::{ResolvedPackage, ResolvedPaths, ResolvedResource};
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

/// The action for a missing configured remote source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingSourceAction {
    /// Acquire the contents before collecting resources.
    Install,
    /// Omit this source.
    Skip,
    /// Fail resolution with the original source spelling.
    Error,
}

/// An operation-local missing-source callback with a borrowed future.
type MissingCallback<'a> = dyn Fn(&str) -> PackageFuture<'a, MissingSourceAction> + 'a;

/// The configuration scope selected for source lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstalledSourceScope {
    /// Sources configured in user settings.
    User,
    /// Sources configured in project settings.
    Project,
}
impl InstalledSourceScope {
    /// Widens an installed scope for internal acquisition and lookup.
    fn resource_scope(self) -> SourceScope {
        match self {
            Self::User => SourceScope::User,
            Self::Project => SourceScope::Project,
        }
    }
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
    /// A caller's offline selection, or the ambient flag when absent.
    offline: Option<bool>,
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
            offline: None,
        }
    }

    /// Selects offline mode from the caller instead of the ambient environment.
    ///
    /// An absent selection keeps reading `MAESTRO_OFFLINE` when an operation runs.
    #[must_use]
    pub fn with_offline(mut self, offline: Option<bool>) -> Self {
        self.offline = offline;
        self
    }

    /// Whether this operation suppresses remote work.
    fn offline(&self) -> bool {
        maestro_text::is_offline_selected(self.offline)
    }
}

/// Source acquisition, removal and configured-source operations.
pub trait PackageManager {
    /// Resolves configured packages, local entries and automatic resources.
    /// The borrowed callback decides missing remote acquisition after the offline check.
    /// Without one, online resolution installs missing contents.
    /// See the [configured resource guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/packages.md#configured-resource-resolution).
    /// # Errors
    /// Returns selected configuration, callback, acquisition or required path failures.
    fn resolve<'a>(
        &'a self,
        on_missing: Option<&'a MissingCallback<'a>>,
    ) -> PackageFuture<'a, ResolvedPaths>;
    /// Resolves explicit sources sequentially, acquiring missing remote contents when online.
    /// npm pins require acquisition when the literal installed version differs.
    /// Only existing, unpinned, online temporary Git refreshes.
    /// Refresh failures are suppressed before resource collection; acquisition failures propagate.
    /// See the [package guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/packages.md).
    /// # Errors
    /// Returns required path, acquisition or remote resource-discovery failures.
    fn resolve_extension_sources<'a>(
        &'a self,
        sources: &'a [String],
        scope: Option<SourceScope>,
    ) -> PackageFuture<'a, ResolvedPaths>;
    /// Appends a source or replaces the first identity match when its parsed pin changes.
    /// Other raw entry fields stay in place; `None` means user scope.
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
    /// Inspects an explicitly pinned managed Git source without changing its checkout.
    /// Missing contents or refs are reported as absent commits, not clean checkouts.
    /// # Errors
    /// Returns invalid source, unsafe checkout, filesystem or Git command failures.
    fn verify_pin<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, GitPinStatus>;
    /// Restores a managed Git pin, removing tracked drift and all untracked files.
    /// Never edits settings or accepts local sources or caller-selected paths.
    /// Missing contents are acquired online; failed repairs may reinstall online.
    /// # Errors
    /// Returns invalid source, unsafe checkout, offline acquisition or native failures.
    fn restore_pin<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Lists Git tags and their peeled commit IDs without selecting a release.
    /// Offline mode refuses remote work rather than returning stale results.
    /// # Errors
    /// Returns invalid source, offline or Git capture failures.
    fn list_releases<'a>(&'a self, source: &'a str) -> PackageFuture<'a, Vec<GitRelease>>;
    /// Reads relative files from a pinned Git candidate in isolated temporary staging.
    /// Results follow input order; a missing file is `None`, and bytes are not decoded.
    /// No dependencies, builds or extensions run, and no installed checkout is changed.
    /// # Errors
    /// Returns invalid source/path, offline, Git, read or cleanup failures.
    fn read_candidate_files<'a>(
        &'a self,
        source: &'a str,
        paths: &'a [String],
    ) -> PackageFuture<'a, Vec<Option<Vec<u8>>>>;
    /// Installs, then adds the source to settings; an existing entry is not an error.
    /// Nothing is flushed; a failed settings edit does not undo acquired contents.
    /// # Errors
    /// Returns installation failures or the settings edit's failure.
    fn install_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Updates matching configured registrations; absent or empty input selects all.
    /// Does not persist settings; matched local and pinned sources are skipped.
    /// # Errors
    /// Returns selection, callback, filesystem, command or worker-admission failures.
    fn update(self: Rc<Self>, source: Option<&str>) -> PackageFuture<'static, ()>
    where
        Self: 'static;
    /// Removes a source's managed contents; `None` means user scope.
    /// # Errors
    /// Returns callback, path, filesystem or command failures.
    fn remove<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()>;
    /// Removes the contents, then the settings entries; returns whether any entry was removed.
    /// Nothing is flushed; a failed settings edit does not undo acquired contents.
    /// # Errors
    /// Returns removal failures or the settings edit's failure.
    fn remove_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, bool>;
}
impl<O: PackageOperations> PackageManager for DefaultPackageManager<O> {
    fn resolve<'a>(
        &'a self,
        on_missing: Option<&'a MissingCallback<'a>>,
    ) -> PackageFuture<'a, ResolvedPaths> {
        Box::pin(self.resolve_configured(on_missing))
    }
    fn resolve_extension_sources<'a>(
        &'a self,
        sources: &'a [String],
        scope: Option<SourceScope>,
    ) -> PackageFuture<'a, ResolvedPaths> {
        Box::pin(async move { self.resolve_sources(sources, scope).await })
    }
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
        Box::pin(async move { self.acquire(source, scope).await.map(drop) })
    }
    fn verify_pin<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, GitPinStatus> {
        Box::pin(self.inspect_pin(source, scope))
    }
    fn restore_pin<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()> {
        Box::pin(self.repair_pin(source, scope))
    }
    fn list_releases<'a>(&'a self, source: &'a str) -> PackageFuture<'a, Vec<GitRelease>> {
        Box::pin(self.git_releases(source))
    }
    fn read_candidate_files<'a>(
        &'a self,
        source: &'a str,
        paths: &'a [String],
    ) -> PackageFuture<'a, Vec<Option<Vec<u8>>>> {
        Box::pin(self.inspect_candidate_files(source, paths))
    }
    fn install_and_persist<'a>(
        &'a self,
        source: &'a str,
        scope: Option<InstalledSourceScope>,
    ) -> PackageFuture<'a, ()> {
        Box::pin(self.acquire_and_persist(source, scope))
    }
    fn update(self: Rc<Self>, source: Option<&str>) -> PackageFuture<'static, ()>
    where
        Self: 'static,
    {
        Box::pin(self.update_configured(source.map(str::to_owned)))
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
