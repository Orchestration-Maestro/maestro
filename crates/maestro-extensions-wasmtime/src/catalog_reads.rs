//! Live catalog reads run on the local owner and return owned model values.
use crate::local_resources::{Lease, Message};
use maestro_extensions::{Model, ModelRegistry, ResolvedRequestAuth};
use serde::Serialize;
use std::sync::{Arc, RwLock, mpsc};

/// A catchable result after the selected catalog operation.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Synchronous operations on the independently retained catalog.
pub(crate) enum Request {
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
            Self::All(reply) | Self::Available(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Find(_, _, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Selects current domain data only when the local pump consumes the request.
    pub(crate) fn answer(self, catalog: &ModelRegistry) {
        match self {
            Self::All(reply) => {
                let list = catalog.get_all();
                let result = list
                    .read()
                    .map_err(|error| error.to_string())
                    .and_then(|models| encode_models(&models));
                let _ = reply.send(result);
            }
            Self::Available(reply) => {
                let result = catalog
                    .get_available()
                    .map_err(|error| error.to_string())
                    .and_then(|models| encode_models(&models));
                let _ = reply.send(result);
            }
            Self::Find(provider, id, reply) => {
                let result = catalog
                    .find(&provider, &id)
                    .map(|model| {
                        let model = model.read().map_err(|error| error.to_string())?;
                        serde_json::to_string(&*model).map_err(|error| error.to_string())
                    })
                    .transpose();
                let _ = reply.send(result);
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
    /// Resolves an owned model through the original catalog and caller's local effects.
    pub(crate) async fn auth(&self, model: Model) -> Result<ResolvedRequestAuth, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::Auth(id, Box::new(model), reply, admission)
            })
            .await
    }
}
