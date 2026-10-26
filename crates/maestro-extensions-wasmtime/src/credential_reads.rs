//! Requests use the retained native credential owner, not a serialized snapshot.
use crate::local_resources::{Lease, Message};
use maestro_extensions::{AuthStorage, DiagnosticErrorInfo};
use std::sync::{Arc, mpsc::Sender};

/// Catchable credential operation result.
type Reply<T> = Sender<Result<T, String>>;

/// Operations on the original accepted credential state.
pub(crate) enum Request {
    /// Installs a runtime-only key.
    Runtime(String, String, Reply<()>),
    /// Removes a runtime-only key.
    RemoveRuntime(String, Reply<()>),
    /// Reloads and records backend failures.
    Reload(Reply<()>),
    /// Reads one typed credential.
    Get(String, Reply<Option<String>>),
    /// Accepts and persists one credential.
    Set(String, String, Reply<()>),
    /// Removes one credential.
    Remove(String, Reply<()>),
    /// Reads provider names in insertion order.
    List(Reply<Vec<String>>),
    /// Checks stored presence, including untyped values.
    Has(String, Reply<bool>),
    /// Checks configured authentication without token refresh.
    HasAuth(String, Reply<bool>),
    /// Reads non-secret source metadata.
    Status(String, Reply<String>),
    /// Reads the typed credential view.
    All(Reply<String>),
    /// Drains persistence and load errors.
    Errors(Reply<String>),
    /// Logs out one provider.
    Logout(String, Reply<()>),
}

impl Request {
    /// Returns an owner failure without stranding a waiting import.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Runtime(_, _, reply)
            | Self::RemoveRuntime(_, reply)
            | Self::Reload(reply)
            | Self::Set(_, _, reply)
            | Self::Remove(_, reply)
            | Self::Logout(_, reply) => send(&reply, Err::<(), _>(error)),
            Self::Get(_, reply) => send(&reply, Err::<Option<String>, _>(error)),
            Self::List(reply) => send(&reply, Err::<Vec<String>, _>(error)),
            Self::Has(_, reply) | Self::HasAuth(_, reply) => send(&reply, Err::<bool, _>(error)),
            Self::Status(_, reply) | Self::All(reply) | Self::Errors(reply) => {
                send(&reply, Err::<String, _>(error));
            }
        }
    }

    /// Calls the native credential algorithms outside the resource-table borrow.
    pub(crate) fn answer(self, owner: &AuthStorage) {
        match self {
            Self::Runtime(provider, key, reply) => {
                owner.set_runtime_api_key(&provider, &key);
                send(&reply, Ok::<_, String>(()));
            }
            Self::RemoveRuntime(provider, reply) => {
                owner.remove_runtime_api_key(&provider);
                send(&reply, Ok::<_, String>(()));
            }
            Self::Reload(reply) => {
                owner.reload();
                send(&reply, Ok::<_, String>(()));
            }
            Self::Get(provider, reply) => {
                let result = owner.get(&provider).map(|value| encode(&value)).transpose();
                send(&reply, result);
            }
            Self::Set(provider, value, reply) => {
                let result =
                    crate::writer_reads::decode(&value).map(|value| owner.set(&provider, value));
                send(&reply, result);
            }
            Self::Remove(provider, reply) => {
                owner.remove(&provider);
                send(&reply, Ok::<_, String>(()));
            }
            Self::List(reply) => send(&reply, Ok::<_, String>(owner.list())),
            Self::Has(provider, reply) => send(&reply, Ok::<_, String>(owner.has(&provider))),
            Self::HasAuth(provider, reply) => send(&reply, owner.has_auth(&provider)),
            Self::Status(provider, reply) => {
                let result = owner
                    .get_auth_status(&provider)
                    .map_err(|error| error.to_string())
                    .and_then(|value| encode(&value));
                send(&reply, result);
            }
            Self::All(reply) => send(&reply, encode(&owner.get_all())),
            Self::Errors(reply) => {
                let errors: Vec<_> = owner
                    .drain_errors()
                    .into_iter()
                    .map(|error| DiagnosticErrorInfo::error(error.to_string()))
                    .collect();
                send(&reply, encode(&errors));
            }
            Self::Logout(provider, reply) => {
                owner.logout(&provider);
                send(&reply, Ok::<_, String>(()));
            }
        }
    }
}

/// Encodes the existing shared record without a smaller intermediate projection.
fn encode(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}
/// Sends the original operation's catchable failure text.
fn send<T, E: std::fmt::Display>(reply: &Reply<T>, result: Result<T, E>) {
    let _ = reply.send(result.map_err(|error| error.to_string()));
}

/// Store-safe lease of the independently retained credential owner.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
impl Endpoint {
    /// Resolves through the same credential effects that the creating catalog retained.
    pub(crate) async fn key(
        &self,
        provider: String,
        include_fallback: Option<bool>,
    ) -> Result<Option<String>, String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::CredentialKey(id, (provider, include_fallback), reply, admission)
            })
            .await
    }
}
