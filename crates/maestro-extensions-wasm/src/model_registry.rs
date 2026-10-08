//! The model catalog queries an extension can make.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

pub use crate::bindings::maestro::extension::session::{RequestCredentials, ResolvedRequestAuth};
use crate::models::Model;
use crate::types::{ExtensionFuture, ExtensionResult};

port! {
    /// Catalog capability behind a [`ModelRegistry`].
    ModelRegistryPort for ModelRegistry via 0 {
        /// Every model of the catalog.
        fn get_all() -> Vec<Model>;
        /// The models that have credentials configured.
        fn get_available() -> Vec<Model>;
        /// The model of a provider with the given identifier, when there is one.
        fn find(provider: &str, model_id: &str) -> Option<Model>;
    }
    extra {
        /// Resolves the credentials and headers of a model. A rejected resolution is a
        /// successful call that returns the rejection; the future fails only when the
        /// capability itself fails.
        fn get_api_key_and_headers(&self, model: Model) -> ExtensionFuture<'_, ResolvedRequestAuth>;
    }
}

/// The model catalog.
#[derive(Clone)]
pub struct ModelRegistry(Rc<dyn ModelRegistryPort>);

impl ModelRegistry {
    /// Wraps the host's catalog.
    #[must_use]
    pub fn new(port: Rc<dyn ModelRegistryPort>) -> Self {
        Self(port)
    }

    /// Resolves the credentials and headers of a model.
    #[must_use]
    pub fn get_api_key_and_headers(
        &self,
        model: Model,
    ) -> ExtensionFuture<'_, ResolvedRequestAuth> {
        self.0.get_api_key_and_headers(model)
    }
}
