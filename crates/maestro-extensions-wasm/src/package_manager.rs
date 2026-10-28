//! Package descriptors, acquisition and pin integrity through the host's package owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use crate::bindings::maestro::extension::host::OperationScope;
use crate::bindings::maestro::extension::host::{HostOperation, PackageRequest, PackageSources};
use crate::{ExtensionAPI, ExtensionFuture};
pub use maestro_package::GitPinStatus;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

/// A selected package in the host operation's scope.
pub type ResolvedPackage = maestro_package::ResolvedPackage<OperationScope>;

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
    /// Returns typed descriptors, including packages without resources.
    #[must_use]
    pub fn resolve(&self) -> ExtensionFuture<'_, Vec<ResolvedPackage>> {
        decode(
            self.api
                .call_host_operation(HostOperation::ResolvePackages(None)),
        )
    }
    /// Resolves the supplied sources in one scope, as `resolve` describes them.
    ///
    /// The sources are not written to settings.
    #[must_use]
    pub fn resolve_extension_sources(
        &self,
        sources: Vec<String>,
        scope: OperationScope,
    ) -> ExtensionFuture<'_, Vec<ResolvedPackage>> {
        decode(
            self.api
                .call_host_operation(HostOperation::ResolvePackages(Some(PackageSources {
                    sources,
                    scope,
                }))),
        )
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
    /// The typed observations expose cleanliness through `GitPinStatus::is_clean`.
    #[must_use]
    pub fn verify_pin(
        &self,
        source: &str,
        scope: OperationScope,
    ) -> ExtensionFuture<'_, GitPinStatus> {
        decode(
            self.api
                .call_host_operation(HostOperation::VerifyPin(request(source, scope))),
        )
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

/// Rejects malformed fixed-schema results at the guest boundary.
fn decode<T: DeserializeOwned + 'static>(
    call: ExtensionFuture<'_, Value>,
) -> ExtensionFuture<'_, T> {
    Box::pin(async move { serde_json::from_value(call.await?).map_err(|error| error.to_string()) })
}

impl Serialize for OperationScope {
    /// Uses the native owner's persisted scope spelling.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match self {
            Self::User => "user",
            Self::Project => "project",
        })
    }
}
impl<'de> Deserialize<'de> for OperationScope {
    /// Admits only the two scopes the host operation can address.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "user" => Ok(Self::User),
            "project" => Ok(Self::Project),
            scope => Err(serde::de::Error::unknown_variant(
                scope,
                &["user", "project"],
            )),
        }
    }
}
