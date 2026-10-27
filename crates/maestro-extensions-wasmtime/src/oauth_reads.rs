//! OAuth calls use the retained implementation, not a later registry lookup.
use crate::local_resources::Lease;
use maestro_extensions::OAuthProviderHandle;
use std::{
    borrow::Cow,
    sync::{Arc, mpsc::Sender},
};

/// Requests on one original OAuth implementation.
pub(crate) enum Request {
    /// Reads identity, label and optional callback-server metadata.
    Info(Sender<Result<String, String>>),
    /// Applies the native optional model transformation.
    Models(String, Sender<Result<String, String>>),
    /// Converts open credentials to a request key.
    Key(String, Sender<Result<String, String>>),
}
impl Request {
    /// Invokes the original native implementation outside the capability-table borrow.
    pub(crate) fn answer(self, provider: &OAuthProviderHandle) {
        let (result, reply) = match self {
            Self::Info(reply) => (
                serde_json::to_string(&(
                    provider.id(),
                    provider.name(),
                    provider.uses_callback_server(),
                ))
                .map_err(|error| error.to_string()),
                reply,
            ),
            Self::Models(data, reply) => {
                let result = crate::writer_reads::decode(&data).and_then(
                    |(models, credentials): (Vec<maestro_extensions::Model>, _)| {
                        let selected = provider
                            .modify_models(&models, &credentials)
                            .transpose()
                            .map_err(|error| error.to_string())?;
                        serde_json::to_string(&selected).map_err(|error| error.to_string())
                    },
                );
                (result, reply)
            }
            Self::Key(data, reply) => {
                let result = crate::writer_reads::decode(&data).and_then(|credentials| {
                    provider
                        .get_api_key(&credentials)
                        .map(Cow::into_owned)
                        .map_err(|error| error.to_string())
                });
                (result, reply)
            }
        };
        let _ = reply.send(result);
    }
    /// Settles a read whose original native resource has ended.
    pub(crate) fn reject(self) {
        let (Self::Info(reply) | Self::Models(_, reply) | Self::Key(_, reply)) = self;
        let _ = reply.send(Err("extension OAuth provider resource ended".into()));
    }
}
/// Store-safe ownership of one independently retained provider.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
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
