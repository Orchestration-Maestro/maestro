//! Command operations selected on consumption and settled by the local pump.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    context_reads::Endpoint,
    guest_calls::Handle,
    local_resources::{Work, held::SessionCommands},
    session_calls::{self, NewSession, Switch},
};
use maestro_extensions::{EventError, types::SessionChangeResult};
use std::future::Future;
use tokio::sync::oneshot;

/// A catchable asynchronous import result.
type Reply<T> = oneshot::Sender<Result<T, String>>;

/// Operations available only through the original command context. Each selects its native
/// operation when the local pump consumes the call and keeps the lease's admission until the
/// operation settles.
impl Endpoint {
    /// Waits for the selected agent operation.
    pub(crate) async fn wait_for_idle(&self) -> Result<(), String> {
        self.0
            .admit::<SessionCommands, _, _>(|context, _| Ok(settled(context.wait_for_idle())))
            .await
    }

    /// Reloads through the selected application operation.
    pub(crate) async fn reload(&self) -> Result<(), String> {
        self.0
            .admit::<SessionCommands, _, _>(|context, _| Ok(settled(context.reload())))
            .await
    }

    /// Creates a session with original setup and continuation operands.
    pub(crate) async fn new_session(
        &self,
        options: NewSession,
    ) -> Result<SessionChangeResult, String> {
        self.0
            .admit::<SessionCommands, _, _>(move |context, registry| {
                Ok(settled(context.new_session(options.options(registry))))
            })
            .await
    }

    /// Forks at the exact raw entry identity with optional data and continuation.
    pub(crate) async fn fork(
        &self,
        id: String,
        data: Option<String>,
        callback: Option<Handle>,
    ) -> Result<SessionChangeResult, String> {
        self.0
            .admit::<SessionCommands, _, _>(move |context, registry| {
                session_calls::fork_options(data.as_deref(), callback, registry)
                    .map(|options| settled(context.fork(id, options)))
            })
            .await
    }

    /// Switches with independently absent versus present-empty continuation options.
    pub(crate) async fn switch_session(
        &self,
        path: String,
        options: Option<Switch>,
    ) -> Result<SessionChangeResult, String> {
        self.0
            .admit::<SessionCommands, _, _>(move |context, registry| {
                let options = options.map(|options| options.options(registry));
                Ok(settled(context.switch_session(path, options)))
            })
            .await
    }

    /// Navigates to the exact target with independently optional tree choices.
    pub(crate) async fn navigate_tree(
        &self,
        id: String,
        options: Option<maestro_extensions::NavigateTreeOptions>,
    ) -> Result<SessionChangeResult, String> {
        self.0
            .admit::<SessionCommands, _, _>(move |context, _| {
                Ok(settled(context.navigate_tree(id, options)))
            })
            .await
    }
}

/// Keeps the domain operation's unchanged error text as the catchable failure.
async fn settled<T>(work: impl Future<Output = Result<T, EventError>>) -> Result<T, String> {
    work.await.map_err(|error| error.to_string())
}

/// Completes the original reply with the domain operation's unchanged error text.
pub(crate) fn settle<T: 'static>(
    work: impl Future<Output = Result<T, EventError>> + 'static,
    reply: Reply<T>,
) -> Work {
    Box::pin(async move {
        let _ = reply.send(settled(work).await);
    })
}
