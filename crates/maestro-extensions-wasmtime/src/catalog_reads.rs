//! Live catalog operations run on the local owner and return owned model values.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::import_route::Route;
use crate::local_resources::{Lease, held::Catalog};
use maestro_extensions::{Model, ModelRegistry, ResolvedRequestAuth};
use serde::Serialize;
use std::sync::{Arc, RwLock};

/// Encodes the descriptors through their read guards, without copying them.
fn encode_models<T: Serialize>(models: &[Arc<RwLock<T>>]) -> Result<String, String> {
    let guards = models
        .iter()
        .map(|model| model.read().map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let models: Vec<&T> = guards.iter().map(|model| &**model).collect();
    serde_json::to_string(&models).map_err(|error| error.to_string())
}

/// Store-owned identity of the original live catalog.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Retains the catalog's original credentials and command effects.
    pub(crate) fn credentials(&self) -> Result<crate::credential_reads::Endpoint, String> {
        self.0.run::<Catalog, _>(|catalog, registry| {
            registry.credentials(
                catalog.auth_storage().clone(),
                catalog.config_values().clone(),
            )
        })
    }

    /// Reads the current full descriptor list.
    pub(crate) fn get_all(&self) -> Result<String, String> {
        self.0.run::<Catalog, _>(|catalog, _| all(catalog))
    }

    /// Finds the first exact provider and model identity.
    pub(crate) fn find(&self, provider: String, id: String) -> Result<Option<String>, String> {
        self.0
            .run::<Catalog, _>(move |catalog, _| find(catalog, &provider, &id))
    }

    /// Reads the current model-file load diagnostic.
    pub(crate) fn get_error(&self) -> Result<Option<String>, String> {
        self.0
            .run::<Catalog, _>(|catalog, _| Ok(catalog.get_error()))
    }

    /// Reads the existing provider display-name precedence.
    pub(crate) fn get_provider_display_name(&self, provider: String) -> Result<String, String> {
        self.0
            .run::<Catalog, _>(move |catalog, _| Ok(catalog.get_provider_display_name(&provider)))
    }

    /// Reads descriptors with configured authentication, without refreshing it.
    pub(crate) fn get_available(&self) -> Route {
        self.0.run_routed::<Catalog, _>(|catalog, _| {
            catalog
                .get_available()
                .map_err(|error| error.to_string())
                .and_then(|models| encode_models(&models))
        })
    }

    /// Reloads the catalog and replays its retained providers.
    pub(crate) fn refresh(&self) -> Route {
        self.0.run_routed::<Catalog, _>(|catalog, _| {
            catalog.refresh().map_err(|error| error.to_string())
        })
    }

    /// Checks authentication for an independently supplied model.
    pub(crate) fn has_configured_auth(&self, model: String) -> Route {
        self.0.run_routed::<Catalog, _>(move |catalog, _| {
            model_record(&model).and_then(|model| {
                catalog
                    .has_configured_auth(&model)
                    .map_err(|error| error.to_string())
            })
        })
    }

    /// Reads non-secret provider authentication metadata.
    pub(crate) fn get_provider_auth_status(&self, provider: String) -> Route {
        self.0.run_routed::<Catalog, _>(move |catalog, _| {
            catalog
                .get_provider_auth_status(&provider)
                .map_err(|error| error.to_string())
                .and_then(|status| encode(&status))
        })
    }

    /// Checks the stored credential kind independently of key selection.
    pub(crate) fn is_using_oauth(&self, model: String) -> Route {
        self.0.run_routed::<Catalog, _>(move |catalog, _| {
            model_record(&model).map(|model| catalog.is_using_oauth(&model))
        })
    }

    /// Registers directly without the extension loading queue.
    pub(crate) fn register_provider(
        &self,
        name: String,
        config: String,
        callbacks: crate::provider_registration::Callbacks,
    ) -> Route {
        self.0.run_routed::<Catalog, _>(move |catalog, registry| {
            crate::provider_registration::config(&config, callbacks, registry).and_then(|config| {
                catalog
                    .register_provider(&name, config)
                    .map_err(|error| error.to_string())
            })
        })
    }

    /// Removes directly through native replay policy.
    pub(crate) fn unregister_provider(&self, name: String) -> Route {
        self.0.run_routed::<Catalog, _>(move |catalog, _| {
            catalog
                .unregister_provider(&name)
                .map_err(|error| error.to_string())
        })
    }

    /// Resolves the selected provider through the original local catalog.
    pub(crate) async fn provider_key(&self, provider: String) -> Result<Option<String>, String> {
        self.0
            .admit::<Catalog, _, _>(move |catalog, _| {
                let catalog = catalog.clone();
                Ok(async move {
                    catalog
                        .get_api_key_for_provider(&provider)
                        .await
                        .map_err(|error| error.to_string())
                })
            })
            .await
    }

    /// Resolves an owned model through the original catalog and caller's local effects.
    pub(crate) async fn auth(&self, model: Model) -> Result<ResolvedRequestAuth, String> {
        self.0
            .admit::<Catalog, _, _>(move |catalog, _| {
                let catalog = catalog.clone();
                let model = Arc::new(RwLock::new(model));
                Ok(async move { Ok(catalog.get_api_key_and_headers(&model).await) })
            })
            .await
    }
}

/// Decodes an independently supplied model without requiring catalog admission.
fn model_record(text: &str) -> Result<Arc<RwLock<Model>>, String> {
    crate::writer_reads::decode(text).map(|model| Arc::new(RwLock::new(model)))
}
/// Encodes native record values without projecting their fields.
fn encode(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}
/// Encodes the current descriptor slots through their read guards.
fn all(catalog: &ModelRegistry) -> Result<String, String> {
    let list = catalog.get_all();
    let models = list.read().map_err(|error| error.to_string())?;
    encode_models(&models)
}
/// Encodes the first exact descriptor match through its current read guard.
fn find(catalog: &ModelRegistry, provider: &str, id: &str) -> Result<Option<String>, String> {
    catalog
        .find(provider, id)
        .map(|model| {
            let model = model.read().map_err(|error| error.to_string())?;
            encode(&*model)
        })
        .transpose()
}
