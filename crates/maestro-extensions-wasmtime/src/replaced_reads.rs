//! Replacement operations retain the actual creating session, not the current API.
use crate::{
    api_reads::delivery,
    local_resources::{Lease, Message, Work},
};
use maestro_extensions::{EventError, ReplacedSessionContext};
use std::{future::Future, sync::Arc};
use tokio::sync::oneshot;

/// A catchable original-session message result.
type Reply = oneshot::Sender<Result<(), String>>;

/// Message operations independent of guarded command-context activity.
pub(crate) enum Request {
    /// Delivers a custom message to the creating session.
    Message(String, Option<String>, Reply),
    /// Delivers text or ordered blocks to the creating session.
    UserMessage(String, Option<String>, Reply),
}

impl Request {
    /// Selects the original message operation before admitting its returned future.
    pub(crate) fn start(self, context: &ReplacedSessionContext) -> Work {
        match self {
            Self::Message(message, options, reply) => {
                let work = delivery(&message, options.as_deref())
                    .map(|(message, options)| context.send_message(message, options));
                settle(work, reply)
            }
            Self::UserMessage(value, options, reply) => {
                let work = delivery(&value, options.as_deref())
                    .map(|(value, options)| context.send_user_message(value, options));
                settle(work, reply)
            }
        }
    }

    /// Settles missing original-session resources without substituting a new session.
    pub(crate) fn reject(self, error: &str) {
        let (Self::Message(_, _, reply) | Self::UserMessage(_, _, reply)) = self;
        let _ = reply.send(Err(error.to_owned()));
    }
}

/// Finishes conversion failures and already-started operations through the same reply.
fn settle(
    work: Result<impl Future<Output = Result<(), EventError>> + 'static, String>,
    reply: Reply,
) -> Work {
    Box::pin(async move {
        let result = match work {
            Ok(work) => work.await.map_err(|error| error.to_string()),
            Err(error) => Err(error),
        };
        let _ = reply.send(result);
    })
}

/// An independently retained original-session capability in the Store.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Retains a distinct guarded command view without retargeting the replacement.
    pub(crate) fn command(&self) -> Result<crate::context_reads::Endpoint, String> {
        self.0.request(Message::ReplacedCommand)
    }

    /// Admits the creating session's message operation through the sole receiver.
    pub(crate) async fn send(&self, request: impl FnOnce(Reply) -> Request) -> Result<(), String> {
        self.0
            .request_async(|id, reply, admission| Message::Replaced(id, request(reply), admission))
            .await
    }
}
