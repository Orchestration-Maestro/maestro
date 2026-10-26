//! Retained model catalog capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::{Imports, session::decode};
use crate::types::finite::encode_value;
use crate::{
    AuthStatus, ExtensionFuture, ExtensionResult, Model, ModelRegistry, ModelRegistryPort,
    ProviderConfig, ResolvedRequestAuth,
};
use std::rc::Rc;
/// Retained host catalog behind the public facade.
struct Registry<I: Imports> {
    /// Host import implementation.
    imports: I,
    /// Independently owned catalog resource.
    resource: I::Registry,
}
/// Wraps an independently owned host catalog.
pub(super) fn registry<I: Imports>(imports: I, resource: I::Registry) -> ModelRegistry {
    ModelRegistry::new(Rc::new(Registry { imports, resource }))
}
impl<I: Imports> ModelRegistryPort for Registry<I> {
    fn auth_storage(&self) -> ExtensionResult<crate::AuthStorage> {
        self.imports
            .registry_auth_storage(&self.resource)
            .map(|resource| super::auth_storage::storage(self.imports.clone(), resource))
    }

    fn get_all(&self) -> ExtensionResult<Vec<Model>> {
        decode(&self.imports.registry_get_all(&self.resource)?)
    }
    fn get_available(&self) -> ExtensionResult<Vec<Model>> {
        decode(&self.imports.registry_get_available(&self.resource)?)
    }
    fn find(&self, provider: &str, model_id: &str) -> ExtensionResult<Option<Model>> {
        self.imports
            .registry_find(&self.resource, provider, model_id)?
            .as_deref()
            .map(decode)
            .transpose()
    }
    fn refresh(&self) -> ExtensionResult<()> {
        self.imports.registry_refresh(&self.resource)
    }
    fn get_error(&self) -> ExtensionResult<Option<String>> {
        self.imports.registry_get_error(&self.resource)
    }
    fn has_configured_auth(&self, model: Model) -> ExtensionResult<bool> {
        self.imports
            .registry_has_configured_auth(&self.resource, &encode_value(&model)?)
    }
    fn get_provider_auth_status(&self, provider: &str) -> ExtensionResult<AuthStatus> {
        decode(
            &self
                .imports
                .registry_get_provider_auth_status(&self.resource, provider)?,
        )
    }
    fn get_provider_display_name(&self, provider: &str) -> ExtensionResult<String> {
        self.imports
            .registry_get_provider_display_name(&self.resource, provider)
    }
    fn is_using_oauth(&self, model: Model) -> ExtensionResult<bool> {
        self.imports
            .registry_is_using_oauth(&self.resource, &encode_value(&model)?)
    }
    fn register_provider(&self, name: &str, config: ProviderConfig) -> ExtensionResult<()> {
        super::provider_registration::register(&self.imports, config, |text, callbacks| {
            self.imports
                .registry_register_provider(&self.resource, name, text, callbacks)
        })
    }
    fn unregister_provider(&self, name: &str) -> ExtensionResult<()> {
        self.imports
            .registry_unregister_provider(&self.resource, name)
    }
    fn get_api_key_for_provider<'a>(
        &'a self,
        provider: &'a str,
    ) -> ExtensionFuture<'a, Option<String>> {
        self.imports
            .registry_get_api_key_for_provider(&self.resource, provider.to_owned())
    }
    fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth> {
        Box::pin(async move {
            let model = encode_value(&model)?;
            self.imports
                .registry_get_api_key_and_headers(&self.resource, model)
                .await
        })
    }
}
