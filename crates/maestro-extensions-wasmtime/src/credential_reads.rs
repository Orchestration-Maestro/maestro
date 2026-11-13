//! Requests use the retained native credential owner, not a serialized snapshot.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    guest_calls::Handle,
    import_route::Route,
    local_resources::{
        Lease, Message,
        held::{AcceptedCredentials, Credentials},
    },
};
use maestro_extensions::{AuthStorage, DiagnosticErrorInfo};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Retains each implementation in the order the credential owner supplies.
fn providers(
    owner: &AuthStorage,
    registry: &crate::local_resources::Registry,
) -> Result<Vec<crate::oauth_reads::Endpoint>, String> {
    owner
        .get_oauth_providers()
        .into_iter()
        .map(|provider| registry.oauth_provider(provider))
        .collect()
}

/// Encodes the existing shared record without a smaller intermediate projection.
fn encode(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}
/// Accepts and persists one credential, rejecting malformed text before any change.
pub(crate) fn set(owner: &AuthStorage, provider: &str, value: &str) -> Result<(), String> {
    crate::writer_reads::decode(value).map(|value| owner.set(provider, value))
}

/// Store-safe lease of the independently retained credential owner.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);
impl Endpoint {
    /// Returns the native implementations in registration order.
    pub(crate) fn get_oauth_providers(&self) -> Result<Vec<crate::oauth_reads::Endpoint>, String> {
        self.0
            .run::<Credentials, _>(|owner, registry| providers(owner, registry))
    }
    /// Replaces only this owner's selected fallback callback.
    pub(crate) fn set_fallback_resolver(
        &self,
        handler: Handle,
        requests: UnboundedSender<Message>,
    ) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner.set_fallback_resolver(handler.fallback(requests));
            Ok(())
        })
    }
    /// Installs a runtime-only key.
    pub(crate) fn set_runtime_api_key(&self, provider: String, key: String) -> Result<(), String> {
        self.0.run::<Credentials, _>(move |owner, _| {
            owner.set_runtime_api_key(&provider, &key);
            Ok(())
        })
    }
    /// Removes a runtime-only key.
    pub(crate) fn remove_runtime_api_key(&self, provider: String) -> Result<(), String> {
        self.0.run::<Credentials, _>(move |owner, _| {
            owner.remove_runtime_api_key(&provider);
            Ok(())
        })
    }
    /// Reloads and records backend failures.
    pub(crate) fn reload(&self) -> Route {
        self.0.run_routed::<Credentials, _>(|owner, _| {
            owner.reload();
            Ok(())
        })
    }
    /// Reads one typed credential.
    pub(crate) fn get(&self, provider: String) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner.get(&provider).map(|value| encode(&value)).transpose()
        })
    }
    /// Accepts and persists one credential.
    pub(crate) fn set(&self, provider: String, value: String) -> Route {
        self.0
            .run_routed::<Credentials, _>(move |owner, _| set(owner, &provider, &value))
    }
    /// Removes one credential.
    pub(crate) fn remove(&self, provider: String) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner.remove(&provider);
            Ok(())
        })
    }
    /// Reads provider names in insertion order.
    pub(crate) fn list(&self) -> Route {
        self.0
            .run_routed::<Credentials, _>(|owner, _| Ok(owner.list()))
    }
    /// Checks stored presence, including untyped values.
    pub(crate) fn has(&self, provider: String) -> Route {
        self.0
            .run_routed::<Credentials, _>(move |owner, _| Ok(owner.has(&provider)))
    }
    /// Checks configured authentication without token refresh.
    pub(crate) fn has_auth(&self, provider: String) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner.has_auth(&provider).map_err(|error| error.to_string())
        })
    }
    /// Reads non-secret source metadata.
    pub(crate) fn get_auth_status(&self, provider: String) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner
                .get_auth_status(&provider)
                .map_err(|error| error.to_string())
                .and_then(|value| encode(&value))
        })
    }
    /// Reads the typed credential view.
    pub(crate) fn get_all(&self) -> Route {
        self.0
            .run_routed::<Credentials, _>(|owner, _| encode(&owner.get_all()))
    }
    /// Drains persistence and load errors.
    pub(crate) fn drain_errors(&self) -> Result<String, String> {
        self.0.run::<Credentials, _>(|owner, _| {
            let errors: Vec<_> = owner
                .drain_errors()
                .into_iter()
                .map(|error| DiagnosticErrorInfo::error(error.to_string()))
                .collect();
            encode(&errors)
        })
    }
    /// Logs out one provider.
    pub(crate) fn logout(&self, provider: String) -> Route {
        self.0.run_routed::<Credentials, _>(move |owner, _| {
            owner.logout(&provider);
            Ok(())
        })
    }
    /// Leaves provider selection, login and persistence with the retained credential owner.
    pub(crate) async fn login(
        &self,
        provider: String,
        controls: crate::oauth_callbacks::Controls,
    ) -> Result<(), String> {
        self.0
            .admit::<Credentials, _, _>(move |owner, registry| {
                let callbacks = controls.callbacks(registry)?;
                let owner = owner.clone();
                Ok(async move {
                    owner
                        .login(&provider, callbacks)
                        .await
                        .map_err(|error| error.to_string())
                })
            })
            .await
    }
    /// Resolves through the same credential effects that the creating catalog retained.
    pub(crate) async fn key(
        &self,
        provider: String,
        include_fallback: Option<bool>,
    ) -> Result<Option<String>, String> {
        self.0
            .admit::<AcceptedCredentials, _, _>(move |accepted, _| {
                let (owner, effects) = (accepted.owner.clone(), accepted.effects.clone());
                Ok(async move {
                    owner
                        .get_api_key(&provider, include_fallback.unwrap_or(true), &*effects)
                        .await
                        .map_err(|error| error.to_string())
                })
            })
            .await
    }
}
