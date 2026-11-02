//! Command operations selected on consumption and settled by the local pump.
use crate::{
    guest_calls::Handle,
    local_resources::{Registry, Work},
    session_calls::{self, NewSession, Switch},
};
use maestro_extensions::{EventError, ExtensionCommandContext, types::SessionChangeResult};
use std::future::Future;
use tokio::sync::oneshot;

/// A catchable asynchronous import result.
type Reply<T> = oneshot::Sender<Result<T, String>>;

/// Operations available only through the original command context.
pub(crate) enum Request {
    /// Waits for the selected agent operation.
    Wait(Reply<()>),
    /// Reloads through the selected application operation.
    Reload(Reply<()>),
    /// Creates a session with original setup and continuation operands.
    NewSession(NewSession, Reply<SessionChangeResult>),
    /// Forks at the exact raw entry identity with optional data and continuation.
    Fork(
        String,
        Option<String>,
        Option<Handle>,
        Reply<SessionChangeResult>,
    ),
    /// Switches with independently absent versus present-empty continuation options.
    Switch(String, Option<Switch>, Reply<SessionChangeResult>),
    /// Navigates to the exact target with independently optional tree choices.
    Navigate(
        String,
        Option<maestro_extensions::NavigateTreeOptions>,
        Reply<SessionChangeResult>,
    ),
}

impl Request {
    /// Selects and starts before returning the future to the shared pump.
    pub(crate) fn start(self, context: &ExtensionCommandContext, registry: &Registry) -> Work {
        match self {
            Self::Wait(reply) => settle(context.wait_for_idle(), reply),
            Self::Reload(reply) => settle(context.reload(), reply),
            Self::NewSession(options, reply) => {
                settle(context.new_session(options.options(registry)), reply)
            }
            Self::Fork(id, data, callback, reply) => {
                match session_calls::fork_options(data.as_deref(), callback, registry) {
                    Ok(options) => settle(context.fork(id, options), reply),
                    Err(error) => Box::pin(async move {
                        let _ = reply.send(Err(error));
                    }),
                }
            }
            Self::Switch(path, options, reply) => settle(
                context.switch_session(path, options.map(|options| options.options(registry))),
                reply,
            ),
            Self::Navigate(id, options, reply) => settle(context.navigate_tree(id, options), reply),
        }
    }

    /// Settles missing or incorrectly typed context resources as catchable errors.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Navigate(_, _, reply)
            | Self::NewSession(_, reply)
            | Self::Fork(_, _, _, reply)
            | Self::Switch(_, _, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Wait(reply) | Self::Reload(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }
}

/// Completes the original reply with the domain operation's unchanged error text.
pub(crate) fn settle<T: 'static>(
    work: impl Future<Output = Result<T, EventError>> + 'static,
    reply: Reply<T>,
) -> Work {
    Box::pin(async move {
        let _ = reply.send(work.await.map_err(|error| error.to_string()));
    })
}
