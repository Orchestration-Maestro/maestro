//! Typed access to one retained capability for the pump's generic call kinds.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Capability, Lease, Message, Registry, Work};
use crate::context_reads::Owner;
use maestro_extensions::{ExtensionCommandContext, ModelRegistry, SharedHandle};
use std::{future::Future, sync::mpsc};

/// One kind of retained capability, with the text its callers receive once it has ended.
pub(crate) trait Held {
    /// What the capability lends to a call on the local owner.
    type Target;
    /// The failure text of a call whose capability has ended or is another kind.
    const ENDED: &'static str;
    /// Lends the capability when the entry holds this kind.
    fn lend(capability: &Capability) -> Option<&Self::Target>;
}

/// The independently retained model catalog.
pub(crate) struct Catalog;

impl Held for Catalog {
    type Target = SharedHandle<ModelRegistry>;
    const ENDED: &'static str = "extension catalog resource ended";
    fn lend(capability: &Capability) -> Option<&Self::Target> {
        match capability {
            Capability::Catalog(catalog) => Some(catalog),
            _ => None,
        }
    }
}

/// The retained context that carries the session-replacing operations.
pub(crate) struct CommandContext;

impl Held for CommandContext {
    type Target = ExtensionCommandContext;
    const ENDED: &'static str = "extension command context resource ended";
    fn lend(capability: &Capability) -> Option<&Self::Target> {
        match capability {
            Capability::Context(Owner::Command(context)) => Some(context),
            _ => None,
        }
    }
}

/// Builds the inline call kind: the call runs on the local owner and replies on its channel.
fn run_message<H: Held, T: Send + 'static>(
    id: u64,
    reply: mpsc::Sender<Result<T, String>>,
    call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
) -> Message {
    Message::Run(
        id,
        Box::new(move |capability, registry| {
            let result = match capability.and_then(H::lend) {
                Some(held) => call(held, registry),
                None => Err(H::ENDED.to_owned()),
            };
            let _ = reply.send(result);
        }),
    )
}

/// Settles the caller's reply with the started work's result, or with the failure that kept the
/// work from starting.
fn settle<T: 'static, F: Future<Output = Result<T, String>> + 'static>(
    started: Result<F, String>,
    reply: tokio::sync::oneshot::Sender<Result<T, String>>,
) -> Work {
    Box::pin(async move {
        let result = match started {
            Ok(work) => work.await,
            Err(error) => Err(error),
        };
        let _ = reply.send(result);
    })
}

impl Lease {
    /// Runs `call` on the local owner against the held capability while the Store thread
    /// waits for the result.
    pub(crate) fn run<H: Held, T: Send + 'static>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        self.request(|id, reply| run_message::<H, T>(id, reply, call))
    }

    /// Runs `call` like [`Lease::run`], for a native operation that may call back into this
    /// instance; the guest drives the returned route until the result arrives.
    pub(crate) fn run_routed<H: Held, T: Into<crate::import_route::Value> + Send + 'static>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<T, String> + Send + 'static,
    ) -> crate::import_route::Route {
        self.route(|id, reply| run_message::<H, T>(id, reply, call))
    }

    /// Starts the work `call` selects on the local owner and awaits its result without
    /// blocking the Store; the lease's admission stays held until the work settles.
    pub(crate) async fn admit<H, T, F>(
        &self,
        call: impl FnOnce(&H::Target, &Registry) -> Result<F, String> + Send + 'static,
    ) -> Result<T, String>
    where
        H: Held,
        T: Send + 'static,
        F: Future<Output = Result<T, String>> + 'static,
    {
        self.request_async(|id, reply, admission| {
            Message::Admit(
                id,
                Box::new(move |capability, registry| -> Work {
                    let started = match capability.and_then(H::lend) {
                        Some(held) => call(held, registry),
                        None => Err(H::ENDED.to_owned()),
                    };
                    settle(started, reply)
                }),
                admission,
            )
        })
        .await
    }
}
