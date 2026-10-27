//! Independently retained credential capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AuthStatus, DiagnosticErrorInfo, ExtensionFuture, ExtensionResult};
pub use maestro_request::auth_storage::{
    ApiKeyCredential, AuthCredential, AuthStorageData, OAuthCredential,
};
pub use maestro_request::oauth::OAuthCredentials;
use std::rc::Rc;

/// Optional controls for request key resolution.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetApiKeyOptions {
    /// Whether to consult the installed fallback; absence means yes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_fallback: Option<bool>,
}

/// A retained key lookup consulted after stored and environment credentials.
pub type FallbackResolver = Rc<dyn Fn(&str) -> ExtensionResult<Option<String>>>;
/// A borrowed update whose replacement is awaited before the backend writes it.
pub type AsyncLockUpdate<'a> =
    Box<dyn FnOnce(Option<String>) -> ExtensionFuture<'a, Option<String>> + 'a>;
/// Author-owned raw credential text with synchronous and awaited exclusion.
pub trait AuthStorageBackend {
    /// Runs the native update on the actual current text, optionally writing its replacement.
    ///
    /// # Errors
    /// Returns acquisition, update or write failure without replacing their messages.
    fn with_lock(
        &self,
        update: &mut dyn FnMut(Option<&str>) -> ExtensionResult<Option<String>>,
    ) -> ExtensionResult<()>;
    /// Awaits the update before writing its optional replacement.
    fn with_lock_async<'a>(&'a self, update: AsyncLockUpdate<'a>) -> ExtensionFuture<'a, ()>;
}

port! {
    /// Operations on one retained credential owner.
    AuthStoragePort for AuthStorage via port {
        errors "Returns the host failure or credential record decoding error.";
        /// Sets a runtime-only key override.
        fn set_runtime_api_key(provider: &str, api_key: &str) -> ();
        /// Removes a runtime-only override.
        fn remove_runtime_api_key(provider: &str) -> ();
        /// Replaces the retained fallback lookup.
        fn set_fallback_resolver(resolver: FallbackResolver) -> ();
        /// Reloads the backend, recording failures for error draining.
        fn reload() -> ();
        /// Reads a typed credential, omitting untyped stored values.
        fn get(provider: &str) -> Option<AuthCredential>;
        /// Accepts and persists one provider credential.
        fn set(provider: &str, credential: AuthCredential) -> ();
        /// Removes one provider credential.
        fn remove(provider: &str) -> ();
        /// Returns callable OAuth providers in native registration order.
        fn get_oauth_providers() -> Vec<Rc<dyn crate::OAuthProviderInterface>>;
        /// Lists all stored provider names in insertion order.
        fn list() -> Vec<String>;
        /// Checks stored presence, including untyped entries.
        fn has(provider: &str) -> bool;
        /// Checks configured authentication without refreshing tokens.
        fn has_auth(provider: &str) -> bool;
        /// Returns non-secret credential source metadata.
        fn get_auth_status(provider: &str) -> AuthStatus;
        /// Returns the typed view of all credentials.
        fn get_all() -> AuthStorageData;
        /// Removes and returns recorded backend failures in order.
        fn drain_errors() -> Vec<DiagnosticErrorInfo>;
        /// Removes the provider's stored credential.
        fn logout(provider: &str) -> ();
    }
    asynchronous {
        /// Resolves a key using the owner's retained effects and source precedence.
        fn get_api_key<'a>(provider: &'a str, options: Option<GetApiKeyOptions>) -> Option<String>;
    }
    owned {
        /// Logs in through the native credential owner, which accepts and persists the result.
        fn login(provider: &str, callbacks: Rc<crate::OAuthLoginCallbacks>) -> ();
    }
}

/// Clones retain the same live credential owner.
#[derive(Clone)]
pub struct AuthStorage {
    /// Supplied live operations.
    port: Rc<dyn AuthStoragePort>,
    /// Adapter-private resource; supplied ports have no transport identity.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) resource: Option<Rc<dyn std::any::Any>>,
}

impl AuthStorage {
    /// Wraps the supplied credential port.
    #[must_use]
    pub fn new(port: Rc<dyn AuthStoragePort>) -> Self {
        Self {
            port,
            #[cfg(any(test, target_arch = "wasm32"))]
            resource: None,
        }
    }
    /// Resolves a key from the retained owner.
    ///
    /// # Errors
    /// Returns the host or credential-resolution failure.
    #[must_use]
    pub fn get_api_key<'a>(
        &'a self,
        provider: &'a str,
        options: Option<GetApiKeyOptions>,
    ) -> ExtensionFuture<'a, Option<String>> {
        self.port.get_api_key(provider, options)
    }
}

impl AuthStorage {
    /// Constructs credentials over the supplied raw-text backend.
    ///
    /// # Errors
    /// Returns the host's construction failure.
    pub fn from_storage(
        api: &crate::ExtensionAPI,
        backend: Rc<dyn AuthStorageBackend>,
    ) -> ExtensionResult<Self> {
        api.auth_storage_from_backend(backend)
    }
    /// Creates file credentials with an explicit path or the application's default.
    ///
    /// # Errors
    /// Returns the host's construction failure.
    pub fn create(api: &crate::ExtensionAPI, path: Option<&str>) -> ExtensionResult<Self> {
        api.create_auth_storage(path)
    }
    /// Creates credentials without file access.
    ///
    /// # Errors
    /// Returns the host's construction failure.
    pub fn in_memory(api: &crate::ExtensionAPI, data: AuthStorageData) -> ExtensionResult<Self> {
        api.auth_storage_in_memory(data)
    }
}
