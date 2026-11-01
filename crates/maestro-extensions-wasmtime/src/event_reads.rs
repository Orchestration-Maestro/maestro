//! Event imports use the supplied facade's original native bus.
use crate::{
    api_reads::Endpoint,
    guest_calls::Handle,
    local_resources::{Admission, Message, Registry, Work, effects::SubscriptionEndpoint},
};
use maestro_extensions::{ExtensionAPI, Local};

/// Operations consumed on the local event owner.
pub(crate) enum Request {
    /// Registers the actual guest listener with the existing native bus.
    On(
        String,
        Handle,
        std::sync::mpsc::Sender<Result<SubscriptionEndpoint, String>>,
    ),
    /// Admits ordinary emission without blocking nested local requests.
    Emit(
        String,
        String,
        tokio::sync::oneshot::Sender<Result<(), String>>,
        Admission,
    ),
}

impl Request {
    /// Starts native work only after request consumption and outside registry borrows.
    pub(crate) fn answer(
        self,
        api: &ExtensionAPI<Local>,
        registry: &Registry,
    ) -> Option<(Work, Admission)> {
        match self {
            Self::On(channel, handler, reply) => {
                let registry_for_handler = registry.clone();
                let id = handler.id();
                let listener: maestro_extensions::EventListener<Local> = std::rc::Rc::new(move |data: std::sync::Arc<serde_json::Value>, emitter: maestro_extensions::CallbackEmitter<'_, Local>| {
                    handler.listener(&registry_for_handler, &data, emitter.frame()).map_err(Into::into)
                });
                registry.listener_identity(&listener, id);
                let subscription = api.events.on(&channel, listener);
                let _ = reply.send(registry.subscription(subscription));
                None
            }
            Self::Emit(channel, data, reply, admission) => {
                let events = api.events.clone();
                let work: Work = Box::pin(async move {
                    let result = emit(&events, &channel, &data).await;
                    let _ = reply.send(result);
                });
                Some((work, admission))
            }
        }
    }

    /// Settles the original waiter if its API capability has ended.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::On(_, _, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Emit(_, _, reply, _) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

impl Endpoint {
    /// Registers on the same local receiver used for every native capability.
    pub(crate) fn event<T>(
        &self,
        request: impl FnOnce(std::sync::mpsc::Sender<Result<T, String>>) -> Request,
    ) -> Result<T, String> {
        self.0
            .request(|id, reply| Message::Events(id, request(reply)))
    }

    /// Preserves data decoding failures as catchable import results.
    pub(crate) async fn emit(&self, channel: String, data: String) -> Result<(), String> {
        self.0
            .request_async(|id, reply, admission| {
                Message::Events(id, Request::Emit(channel, data, reply, admission))
            })
            .await
    }
}

/// Decodes before emission while preserving the native bus's first-poll behavior.
async fn emit(
    events: &maestro_extensions::EventBus<Local>,
    channel: &str,
    data: &str,
) -> Result<(), String> {
    let data = crate::writer_reads::decode(data)?;
    events.emit(channel, data).await;
    Ok(())
}
