//! Live catalog reads run on the local owner and return owned model values.
use crate::local_resources::{Lease, Message};
use maestro_extensions::{Model, ModelRegistry, ResolvedRequestAuth};
use serde::Serialize;
use std::sync::{Arc, RwLock, mpsc};

/// A catchable result after the selected catalog operation.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Synchronous operations on the independently retained catalog.
pub(crate) enum Request {
    /// Retains the catalog's original credentials and command effects.
    Credentials(Reply<crate::credential_reads::Endpoint>),
    /// Reloads the catalog and replays its retained providers.
    Refresh(Reply<()>),
    /// Reads the current model-file load diagnostic.
    Error(Reply<Option<String>>),
    /// Checks authentication for an independently supplied model.
    HasConfiguredAuth(String, Reply<bool>),
    /// Reads non-secret provider authentication metadata.
    Status(String, Reply<String>),
    /// Reads the existing provider display-name precedence.
    Display(String, Reply<String>),
    /// Checks the stored credential kind independently of key selection.
    IsUsingOAuth(String, Reply<bool>),
    /// Registers directly without the extension loading queue.
    Register(
        String,
        String,
        crate::provider_registration::Callbacks,
        Reply<()>,
    ),
    /// Removes directly through native replay policy.
    Unregister(String, Reply<()>),
    /// Reads the current full descriptor list.
    All(Reply<String>),
    /// Reads descriptors with configured authentication, without refreshing it.
    Available(Reply<String>),
    /// Finds the first exact provider and model identity.
    Find(String, String, Reply<Option<String>>),
}

impl Request {
    /// Settles a missing catalog without leaving its caller waiting.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Credentials(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::All(reply)
            | Self::Available(reply)
            | Self::Status(_, reply)
            | Self::Display(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Refresh(reply) | Self::Register(_, _, _, reply) | Self::Unregister(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::HasConfiguredAuth(_, reply) | Self::IsUsingOAuth(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Error(reply) | Self::Find(_, _, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Selects current domain data only when the local pump consumes the request.
    pub(crate) fn answer(
        self,
        catalog: &ModelRegistry,
        registry: &crate::local_resources::Registry,
    ) {
        match self {
            Self::Credentials(reply) => {
                let _ = reply.send(registry.credentials(
                    catalog.auth_storage().clone(),
                    catalog.config_values().clone(),
                ));
            }
            Self::Refresh(reply) => send(&reply, catalog.refresh()),
            Self::Error(reply) => {
                let _ = reply.send(Ok(catalog.get_error()));
            }
            Self::HasConfiguredAuth(model, reply) => {
                let result = model_record(&model).and_then(|model| {
                    catalog
                        .has_configured_auth(&model)
                        .map_err(|error| error.to_string())
                });
                let _ = reply.send(result);
            }
            Self::IsUsingOAuth(model, reply) => {
                let result = model_record(&model).map(|model| catalog.is_using_oauth(&model));
                let _ = reply.send(result);
            }
            Self::Status(provider, reply) => {
                let result = catalog
                    .get_provider_auth_status(&provider)
                    .map_err(|error| error.to_string())
                    .and_then(|status| encode(&status));
                let _ = reply.send(result);
            }
            Self::Display(provider, reply) => {
                let _ = reply.send(Ok(catalog.get_provider_display_name(&provider)));
            }
            Self::Register(name, config, callbacks, reply) => {
                let result = crate::provider_registration::config(&config, callbacks, registry)
                    .and_then(|config| {
                        catalog
                            .register_provider(&name, config)
                            .map_err(|error| error.to_string())
                    });
                let _ = reply.send(result);
            }
            Self::Unregister(name, reply) => send(&reply, catalog.unregister_provider(&name)),
            Self::All(reply) => {
                let _ = reply.send(all(catalog));
            }
            Self::Available(reply) => {
                let result = catalog
                    .get_available()
                    .map_err(|error| error.to_string())
                    .and_then(|models| encode_models(&models));
                let _ = reply.send(result);
            }
            Self::Find(provider, id, reply) => {
                let _ = reply.send(find(catalog, &provider, &id));
            }
        }
    }
}

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
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Resolves the selected provider through the original local catalog.
    pub(crate) async fn provider_key(&self, provider: String) -> Result<Option<String>, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::ProviderKey(id, provider, reply, admission)
            })
            .await
    }

    /// Resolves an owned model through the original catalog and caller's local effects.
    pub(crate) async fn auth(&self, model: Model) -> Result<ResolvedRequestAuth, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::Auth(id, Box::new(model), reply, admission)
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
/// Retains the domain's error message as a catchable import failure.
fn send<T, E: std::fmt::Display>(reply: &Reply<T>, result: Result<T, E>) {
    let _ = reply.send(result.map_err(|error| error.to_string()));
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
