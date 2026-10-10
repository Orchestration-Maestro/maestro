//! Retained model catalog capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::{ExtensionFuture, ExtensionResult, Model};
use indexmap::IndexMap;
use std::rc::Rc;
/// Authentication resolution, distinct from a failed host call.
pub enum ResolvedRequestAuth {
    /// Resolved credentials.
    Ok {
        /// Optional key, retaining present empty strings.
        api_key: Option<String>,
        /// Optional headers in host order.
        headers: Option<IndexMap<String, String>>,
    },
    /// Structured authentication denial.
    Error {
        /// The supplied denial message.
        error: String,
    },
}
/// Host queries available through a retained catalog.
pub trait ModelRegistryPort {
    /// Returns the supplied model list.
    ///
    /// # Errors
    /// Returns the host query failure.
    fn get_all(&self) -> ExtensionResult<Vec<Model>>;
    /// Returns the supplied configured-auth list.
    ///
    /// # Errors
    /// Returns the host query failure.
    fn get_available(&self) -> ExtensionResult<Vec<Model>>;
    /// Finds a descriptor using literal provider and model identifiers.
    ///
    /// # Errors
    /// Returns the host query failure.
    fn find(&self, provider: &str, model_id: &str) -> ExtensionResult<Option<Model>>;
    /// Resolves credentials for the supplied model.
    fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth>;
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
    /// Returns the supplied model list.
    ///
    /// # Errors
    /// Returns the host failure or selected-record decoding error.
    pub fn get_all(&self) -> ExtensionResult<Vec<Model>> {
        self.0.get_all()
    }
    /// Returns the supplied configured-auth list.
    ///
    /// # Errors
    /// Returns the host failure or selected-record decoding error.
    pub fn get_available(&self) -> ExtensionResult<Vec<Model>> {
        self.0.get_available()
    }
    /// Finds a descriptor using literal provider and model identifiers.
    ///
    /// # Errors
    /// Returns the host failure or selected-record decoding error.
    pub fn find(&self, provider: &str, model_id: &str) -> ExtensionResult<Option<Model>> {
        self.0.find(provider, model_id)
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
}
