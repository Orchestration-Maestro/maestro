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

port! {
    /// Operations on one retained credential owner.
    AuthStoragePort for AuthStorage via 0 {
        errors "Returns the host failure or credential record decoding error.";
        /// Sets a runtime-only key override.
        fn set_runtime_api_key(provider: &str, api_key: &str) -> ();
        /// Removes a runtime-only override.
        fn remove_runtime_api_key(provider: &str) -> ();
        /// Reloads the backend, recording failures for error draining.
        fn reload() -> ();
        /// Reads a typed credential, omitting untyped stored values.
        fn get(provider: &str) -> Option<AuthCredential>;
        /// Accepts and persists one provider credential.
        fn set(provider: &str, credential: AuthCredential) -> ();
        /// Removes one provider credential.
        fn remove(provider: &str) -> ();
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
}

/// Clones retain the same live credential owner.
#[derive(Clone)]
pub struct AuthStorage(Rc<dyn AuthStoragePort>);
impl AuthStorage {
    /// Wraps the supplied credential port.
    #[must_use]
    pub fn new(port: Rc<dyn AuthStoragePort>) -> Self {
        Self(port)
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
        self.0.get_api_key(provider, options)
    }
}
