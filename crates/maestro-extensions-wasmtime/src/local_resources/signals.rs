//! The original cancellation's reads and abort observers on the local owner.
use super::{
    Capability, Lease, Registry, effects::SubscriptionEndpoint, held::Signal, pump::State,
};
use crate::guest_calls::{Observer, SynchronousKind};
use futures_util::future::{Either, select};
use maestro_extensions::Cancellation;
use std::{convert::Infallible, rc::Rc, sync::Arc};
use tokio::sync::oneshot;

/// A cancellation resource retains its original signal independently of the context.
#[derive(Clone)]
pub struct SignalEndpoint(pub(crate) Arc<Lease>);

impl SignalEndpoint {
    /// Reads on the local owner through the same request pump as context operations.
    pub(crate) fn aborted(&self) -> Result<bool, String> {
        self.0
            .run::<Signal, _>(|signal, _| Ok(Cancellation::is_aborted(signal)))
    }

    /// Aborts through the existing continuation so relayed signals can call this guest again.
    pub(crate) fn abort(&self) -> crate::import_route::Route {
        self.0.run_routed::<Signal, _>(|signal, _| {
            Cancellation::abort(signal);
            Ok(())
        })
    }

    /// Registers `listener` to run once, through its synchronous export, when aborted.
    pub(crate) fn subscribe(
        &self,
        listener: Observer,
        api: crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        self.0
            .run::<Signal, _>(move |signal, registry| registry.subscribe(signal, listener, &api))
    }
}

impl Registry {
    /// Adds the guest listener as an abort observer and keeps its explicit removal.
    ///
    /// The observer keeps the listener without keeping the Store, this pump or the bus alive,
    /// so a signal that outlives the extension retains none of them. An abort on another thread wakes
    /// the retained waiter, which aborts the signal again here so the observer runs on this
    /// owner; the waiter ends when the observer is released, by removal or after it ran.
    /// A failed guest listener is reported through its extension's bus; abort still
    /// returns and later observers still run.
    fn subscribe(
        &self,
        signal: &Cancellation,
        listener: Observer,
        api: &crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        self.events(api)?;
        let (pump, api) = (self.pump.clone(), api.0.id);
        let (registered, released) = oneshot::channel::<Infallible>();
        let remove: Rc<dyn Fn()> = Rc::from(signal.subscribe(Rc::new(move || {
            let _ = &registered;
            if let Some(state) = pump.upgrade() {
                state.notify_abort(&listener, api);
            }
        })));
        let capability = Capability::SignalSubscription(Rc::clone(&remove));
        let lease = match self.insert(capability) {
            Ok(lease) => lease,
            Err(error) => {
                remove();
                return Err(error);
            }
        };
        let pump = self
            .pump
            .upgrade()
            .ok_or_else(|| "extension resource owner ended".to_owned())?;
        pump.stage(waiter(signal, released));
        Ok(SubscriptionEndpoint(lease))
    }
}

impl State {
    /// Runs one abort listener through the original extension's bus and a fresh invocation
    /// frame, while this pump and its Store are live.
    fn notify_abort(&self, listener: &Observer, api: u64) {
        let capability = self.registry.capability(api);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            return;
        };
        let Ok(admission) = self.registry.admission() else {
            return;
        };
        let Some(listener) = listener.handle(admission) else {
            return;
        };
        if let Err(message) = crate::compact_calls::notify(
            &listener,
            &self.registry,
            &api.events,
            SynchronousKind::Abort,
            String::new(),
        ) {
            api.events
                .report_error("abort", &std::io::Error::other(message));
        }
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
