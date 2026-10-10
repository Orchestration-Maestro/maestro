//! Retained model catalog capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::{Imports, session::decode};
use crate::{
    ExtensionFuture, ExtensionResult, Model, ModelRegistry, ModelRegistryPort, ResolvedRequestAuth,
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
    fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth> {
        Box::pin(async move {
            let model = crate::types::finite::encode_value(&model)?;
            self.imports
                .registry_get_api_key_and_headers(&self.resource, model)
                .await
        })
    }
}
