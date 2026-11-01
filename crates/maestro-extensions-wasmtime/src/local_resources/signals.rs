//! The original cancellation's reads and abort observers on the local owner.
use super::{Capability, Lease, Message, effects::SubscriptionEndpoint, pump::State};
use crate::guest_calls::{Observer, SynchronousKind};
use futures_util::future::{Either, select};
use maestro_extensions::Cancellation;
use std::{convert::Infallible, rc::Rc, sync::Arc, sync::mpsc};
use tokio::sync::oneshot;

/// Catchable results returned to the waiting Store thread.
type Reply<T> = mpsc::Sender<Result<T, String>>;

/// Operations on one retained original cancellation.
pub(crate) enum Request {
    /// Reads whether the signal was aborted.
    Aborted(Reply<bool>),
    /// Aborts this original cancellation.
    Abort(Reply<()>),
    /// Registers a guest listener with the original extension's bus for its callback frame.
    Subscribe(
        Observer,
        crate::api_reads::Endpoint,
        Reply<SubscriptionEndpoint>,
    ),
}

/// A cancellation resource retains its original signal independently of the context.
#[derive(Clone)]
pub(crate) struct SignalEndpoint(pub(super) Arc<Lease>);

impl SignalEndpoint {
    /// Reads on the local owner through the same request pump as context operations.
    pub(crate) fn aborted(&self) -> Result<bool, String> {
        self.0
            .request(|id, reply| Message::Signal(id, Request::Aborted(reply)))
    }

    /// Aborts through the existing continuation so relayed signals can call this guest again.
    pub(crate) fn abort(&self) -> crate::import_route::Route {
        self.0
            .route(|id, reply| Message::Signal(id, Request::Abort(reply)))
    }

    /// Registers `listener` to run once, through its synchronous export, when aborted.
    pub(crate) fn subscribe(
        &self,
        listener: Observer,
        api: crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        self.0
            .request(|id, reply| Message::Signal(id, Request::Subscribe(listener, api, reply)))
    }
}

impl State {
    /// Serves a signal request after releasing the capability-table borrow.
    pub(super) fn signal(&self, id: u64, request: Request) {
        let capability = self.registry.capability(id);
        let signal = match capability.as_deref() {
            Some(Capability::Signal(signal)) => Some(signal),
            _ => None,
        };
        let ended = || "extension signal resource ended".to_owned();
        match request {
            Request::Aborted(reply) => {
                let _ = reply.send(signal.map(Cancellation::is_aborted).ok_or_else(ended));
            }
            Request::Abort(reply) => {
                let _ = reply.send(signal.map(Cancellation::abort).ok_or_else(ended));
            }
            Request::Subscribe(listener, api, reply) => {
                let result = signal
                    .ok_or_else(ended)
                    .and_then(|signal| self.subscribe(signal, listener, &api));
                let _ = reply.send(result);
            }
        }
    }

    /// Adds the guest listener as an abort observer and keeps its explicit removal.
    ///
    /// The observer keeps the listener without keeping the Store, this pump or the bus alive,
    /// so a signal that outlives the extension retains none of them. An abort on another thread wakes
    /// the retained waiter, which aborts the signal again here so the observer runs on this
    /// owner; the waiter ends when the observer is released, by removal or after it ran.
    /// A failed guest listener is not reported yet; abort still returns and later
    /// observers still run.
    fn subscribe(
        &self,
        signal: &Cancellation,
        listener: Observer,
        api: &crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        self.registry.events(api)?;
        let (pump, api) = (self.registry.pump.clone(), api.0.id);
        let (registered, released) = oneshot::channel::<Infallible>();
        let remove: Rc<dyn Fn()> = Rc::from(signal.subscribe(Rc::new(move || {
            let _ = &registered;
            if let Some(state) = pump.upgrade() {
                let _ = state.notify_abort(&listener, api);
            }
        })));
        let capability = Capability::SignalSubscription(Rc::clone(&remove));
        let lease = match self.registry.insert(capability) {
            Ok(lease) => lease,
            Err(error) => {
                remove();
                return Err(error);
            }
        };
        self.stage(waiter(signal, released));
        Ok(SubscriptionEndpoint(lease))
    }
}

impl State {
    /// Runs one abort listener through the original extension's bus and a fresh invocation
    /// frame, while this pump and its Store are live.
    fn notify_abort(&self, listener: &Observer, api: u64) -> Result<(), String> {
        let capability = self.registry.capability(api);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            return Err("extension API resource ended".to_owned());
        };
        let listener = listener
            .handle(self.registry.admission()?)
            .ok_or("extension Store owner ended")?;
        crate::compact_calls::notify(
            &listener,
            &self.registry,
            &api.events,
            SynchronousKind::Abort,
            String::new(),
        )
    }
}

/// Delivers a foreign-thread abort here while the observer stays registered, even after its
/// explicit remover was dropped; `released` ends it once the observer is gone.
fn waiter(signal: &Cancellation, released: oneshot::Receiver<Infallible>) -> super::Work {
    let aborted = signal.cancelled();
    let signal = signal.clone();
    Box::pin(async move {
        if let Either::Left(_) = select(Box::pin(aborted), released).await {
            signal.abort();
        }
    })
}
