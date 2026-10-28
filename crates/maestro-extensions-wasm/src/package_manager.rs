//! Package descriptors, acquisition and pin integrity through the host's package owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use crate::bindings::maestro::extension::host::OperationScope;
use crate::{ExtensionAPI, ExtensionFuture};
use serde_json::Value;

/// The host's package owner; the component copies no resolution or Git logic.
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
    /// Resolves configured packages.
    #[must_use]
    pub fn resolve(&self) -> ExtensionFuture<'_, Value> {
        self.unconfigured()
    }
    /// Resolves the supplied sources in one scope.
    #[must_use]
    pub fn resolve_extension_sources(
        &self,
        sources: Vec<String>,
        scope: OperationScope,
    ) -> ExtensionFuture<'_, Value> {
        drop((sources, scope));
        self.unconfigured()
    }
    /// Acquires a source's contents.
    #[must_use]
    pub fn install(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, ()> {
        let _ = (source, scope);
        self.unconfigured()
    }
    /// Observes a pinned checkout.
    #[must_use]
    pub fn verify_pin(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, Value> {
        let _ = (source, scope);
        self.unconfigured()
    }
    /// Restores a pinned checkout.
    #[must_use]
    pub fn restore_pin(&self, source: &str, scope: OperationScope) -> ExtensionFuture<'_, ()> {
        let _ = (source, scope);
        self.unconfigured()
    }
    /// Fails every call until the host operation is wired.
    fn unconfigured<T: 'static>(&self) -> ExtensionFuture<'_, T> {
        let api = self.api.clone();
        Box::pin(async move {
            drop(api);
            Err("Extension host operations are not configured".into())
        })
    }
}
