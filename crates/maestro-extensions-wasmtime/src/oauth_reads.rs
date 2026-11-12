//! OAuth calls use the retained implementation, not a later registry lookup.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::local_resources::{Lease, held::OAuth};
use std::{borrow::Cow, sync::Arc};

/// Store-safe ownership of one independently retained provider.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Reads identity, label and optional callback-server metadata.
    pub(crate) fn info(&self) -> Result<String, String> {
        self.0.run::<OAuth, _>(|provider, _| {
            serde_json::to_string(&(
                provider.id(),
                provider.name(),
                provider.uses_callback_server(),
            ))
            .map_err(|error| error.to_string())
        })
    }

    /// Applies the native optional model transformation; the provider may call back into this
    /// instance.
    pub(crate) fn modify_models(&self, data: String) -> crate::import_route::Route {
        self.0.run_routed::<OAuth, _>(move |provider, _| {
            crate::writer_reads::decode(&data).and_then(
                |(models, credentials): (Vec<maestro_extensions::Model>, _)| {
                    let selected = provider
                        .modify_models(&models, &credentials)
                        .transpose()
                        .map_err(|error| error.to_string())?;
                    serde_json::to_string(&selected).map_err(|error| error.to_string())
                },
            )
        })
    }

    /// Converts open credentials to a request key; the provider may call back into this
    /// instance.
    pub(crate) fn get_api_key(&self, data: String) -> crate::import_route::Route {
        self.0.run_routed::<OAuth, _>(move |provider, _| {
            crate::writer_reads::decode(&data).and_then(|credentials| {
                provider
                    .get_api_key(&credentials)
                    .map(Cow::into_owned)
                    .map_err(|error| error.to_string())
            })
        })
    }

    /// Starts authorization on the original native owner.
    pub(crate) async fn login(
        &self,
        controls: crate::oauth_callbacks::Controls,
    ) -> Result<String, String> {
        self.0
            .request_async(|id, reply, admission| {
                crate::local_resources::Message::OAuthLogin(id, controls, reply, admission)
            })
            .await
    }
    /// Awaits token rotation on the original native owner and pending-work service.
    pub(crate) async fn refresh(&self, credentials: String) -> Result<String, String> {
        self.0
            .request_async(|id, reply, admission| {
                crate::local_resources::Message::OAuthRefresh(id, credentials, reply, admission)
            })
            .await
    }
}
