//! Package descriptors, acquisition and pin integrity through the host's package owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use crate::bindings::maestro::extension::host::OperationScope;
use crate::bindings::maestro::extension::host::{HostOperation, PackageRequest, PackageSources};
use crate::{ExtensionAPI, ExtensionFuture};
use serde_json::Value;

/// The host's package owner; the component copies no resolution or Git logic.
///
/// Calls work from the factory onwards, without a session or command context.
/// Failures carry the host's own message.
#[derive(Clone)]
pub struct DefaultPackageManager {
    /// The factory's original host.
    api: ExtensionAPI,
}
impl DefaultPackageManager {
    /// Retains the supplied host without touching installed contents.
    #[must_use]
    pub fn new(api: ExtensionAPI) -> Self {
        Self { api }
    }
    /// Resolves the configured packages, the project's entry winning for one identity.
    ///
    /// Each descriptor holds `source`, `identity`, `scope` and `installedPath`; a
    /// package without resources is listed too.
    #[must_use]
    pub fn resolve(&self) -> ExtensionFuture<'_, Value> {
        self.api
            .call_host_operation(HostOperation::ResolvePackages(None))
    }
    /// Resolves the supplied sources in one scope, as `resolve` describes them.
    ///
    /// The sources are not written to settings.
    #[must_use]
    pub fn resolve_extension_sources(
        &self,
        sources: Vec<String>,
        scope: OperationScope,
    ) -> ExtensionFuture<'_, Value> {
        self.api
            .call_host_operation(HostOperation::ResolvePackages(Some(PackageSources {
                sources,
                scope,
            })))
    }
    /// Acquires a source's contents at its pin, without editing settings.
    #[must_use]
    pub fn install(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, ()> {
        let call = self
            .api
            .call_host_operation(HostOperation::Acquire(request(source, scope)));
        Box::pin(async move { call.await.map(drop) })
    }
    /// Observes a pinned Git checkout without changing it.
    ///
    /// The status holds `expectedCommit`, `headCommit`, `changedPaths` and `clean`.
    #[must_use]
    pub fn verify_pin(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, Value> {
        self.api
            .call_host_operation(HostOperation::VerifyPin(request(source, scope)))
    }
    /// Returns a pinned Git checkout to its commit, removing changed and untracked files.
    #[must_use]
    pub fn restore_pin(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, ()> {
        let call = self
            .api
            .call_host_operation(HostOperation::RestorePin(request(source, scope)));
        Box::pin(async move { call.await.map(drop) })
    }
}

/// Keeps the authored spelling, pin included.
fn request(source: &str, scope: OperationScope) -> PackageRequest {
    PackageRequest {
        source: source.to_owned(),
        scope,
    }
}
