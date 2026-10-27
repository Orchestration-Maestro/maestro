//! Retained model catalog capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{AuthStatus, ExtensionFuture, ExtensionResult, Model, ProviderConfig};
pub use maestro_request::model_registry::ResolvedRequestAuth;
use std::rc::Rc;
port! {
    /// Host queries available through a retained catalog.
    ModelRegistryPort for ModelRegistry via 0 {
        errors "Returns the host failure or selected-record decoding error.";
        /// Returns the same credential owner this catalog uses.
        fn auth_storage() -> crate::AuthStorage;
        /// Returns the supplied model list.
        fn get_all() -> Vec<Model>;
        /// Returns the supplied configured-auth list.
        fn get_available() -> Vec<Model>;
        /// Finds a descriptor using literal provider and model identifiers.
        fn find(provider: &str, model_id: &str) -> Option<Model>;
        /// Reloads the host catalog and replays its registered providers.
        fn refresh() -> ();
        /// Returns the catalog's current load error.
        fn get_error() -> Option<String>;
        /// Reports whether the model's provider has configured authentication.
        fn has_configured_auth(model: Model) -> bool;
        /// Returns the provider's authentication status without key values.
        fn get_provider_auth_status(provider: &str) -> AuthStatus;
        /// Returns the provider's display name, or the literal identifier.
        fn get_provider_display_name(provider: &str) -> String;
        /// Reports whether the model's provider uses stored subscription credentials.
        fn is_using_oauth(model: Model) -> bool;
        /// Registers a provider directly in this catalog.
        fn register_provider(name: &str, config: ProviderConfig) -> ();
        /// Unregisters a provider from this catalog.
        fn unregister_provider(name: &str) -> ();
    }
    asynchronous {
        /// Resolves credentials for the supplied model.
        fn get_api_key_and_headers<'a>(model: Model) -> ResolvedRequestAuth;
        /// Resolves the provider's request key.
        fn get_api_key_for_provider<'a>(provider: &'a str) -> Option<String>;
    }
}
/// Retained catalog; clones share its host owner.
#[derive(Clone)]
pub struct ModelRegistry(Rc<dyn ModelRegistryPort>);
impl ModelRegistry {
    /// Wraps the supplied catalog port.
    #[must_use]
    pub fn new(port: Rc<dyn ModelRegistryPort>) -> Self {
        Self(port)
    }
    /// Resolves credentials for the supplied model.
    ///
    /// # Errors
    /// Returns the host failure or authored-model encoding error.
    #[must_use]
    pub fn get_api_key_and_headers(
        &self,
        model: Model,
    ) -> ExtensionFuture<'_, ResolvedRequestAuth> {
        self.0.get_api_key_and_headers(model)
    }
    /// Resolves the provider's request key.
    ///
    /// # Errors
    /// Returns the host failure.
    #[must_use]
    pub fn get_api_key_for_provider<'a>(
        &'a self,
        provider: &'a str,
    ) -> ExtensionFuture<'a, Option<String>> {
        self.0.get_api_key_for_provider(provider)
    }
}

impl ModelRegistry {
    /// Creates a file catalog over the supplied live credential owner.
    ///
    /// # Errors
    /// Returns the host's construction failure.
    pub fn create(
        api: &crate::ExtensionAPI,
        auth_storage: crate::AuthStorage,
        path: Option<&str>,
    ) -> ExtensionResult<Self> {
        api.create_model_registry(auth_storage, path)
    }
    /// Creates a catalog without file access.
    ///
    /// # Errors
    /// Returns the host's construction failure.
    pub fn in_memory(
        api: &crate::ExtensionAPI,
        auth_storage: crate::AuthStorage,
    ) -> ExtensionResult<Self> {
        api.in_memory_model_registry(auth_storage)
    }
}
