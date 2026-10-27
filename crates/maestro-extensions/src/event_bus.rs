//! Ordered channels shared by independently identified extensions.
use crate::{ExtensionExecution, types::lock};
use maestro_models::{Execution, Threaded};
use serde_json::Value;
use std::{
    error::Error,
    future::Future,
    ops::Deref,
    sync::{Arc, Mutex},
};

/// A native listener failure.
pub type EventError = Box<dyn Error + Send + Sync + 'static>;
/// An owned asynchronous listener continuation.
pub type EventTail<E = Threaded> = <E as Execution>::Future<'static, Result<(), EventError>>;
/// Enqueues work on the emitting code's serial cooperative executor.
///
/// Must not poll inline or concurrently with the emitting synchronous segment.
/// A genuine suspension must allow queued work to progress.
pub type TailSpawner<E = Threaded> = <E as Execution>::Shared<<E as ExtensionExecution>::Spawner>;
/// Receives a complete attributed diagnostic, including its final newline.
pub type ErrorReporter<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Reporter>;
/// Runs a synchronous prefix and optionally returns an owned continuation.
pub type EventListener<E = Threaded> =
    <E as Execution>::Shared<<E as ExtensionExecution>::Listener>;
/// One independently removable listener registration.
pub(crate) struct Registration<E: ExtensionExecution> {
    /// Literal channel name.
    pub(crate) channel: String,
    /// Identity of the subscribing extension.
    pub(crate) owner: Arc<()>,
    /// Caller-owned listener.
    pub(crate) handler: EventListener<E>,
}
/// Shared subscriptions and caller-supplied execution facilities.
pub(crate) struct Shared<E: ExtensionExecution> {
    /// Ordered registrations, locked only for collection operations.
    registrations: Mutex<Vec<E::Shared<Registration<E>>>>,
    /// Serial continuation enqueue facility.
    spawn: TailSpawner<E>,
    /// Diagnostic destination.
    report: ErrorReporter<E>,
}
/// An extension's shared channel capability.
pub struct EventBus<E: ExtensionExecution = Threaded> {
    /// Bus shared by all extension handles.
    pub(crate) shared: E::Shared<Shared<E>>,
    /// Identity retained by cloned handles.
    pub(crate) owner: Arc<()>,
}
/// Owns the ability to clear all subscriptions on a shared bus.
pub struct EventBusController<E: ExtensionExecution = Threaded>(EventBus<E>);
/// An explicit, idempotent remover; dropping it leaves the listener registered.
pub struct Subscription<E: ExtensionExecution = Threaded> {
    /// Bus containing the registration.
    shared: E::Shared<Shared<E>>,
    /// Registration identity without ownership of the listener.
    registration: E::Weak<Registration<E>>,
}
/// Creates an empty shared bus with caller-owned execution and diagnostics.
#[must_use]
pub fn create_event_bus(spawn: TailSpawner, report: ErrorReporter) -> EventBusController {
    create_event_bus_for::<Threaded>(spawn, report)
}
/// Creates an empty shared bus whose callbacks use the caller's execution family.
///
/// [`Local`](maestro_models::Local) listeners, tails, spawners and reporters may
/// capture single-thread values such as terminal components.
#[must_use]
pub fn create_event_bus_for<E: ExtensionExecution>(
    spawn: TailSpawner<E>,
    report: ErrorReporter<E>,
) -> EventBusController<E> {
    EventBusController(EventBus {
        shared: E::share(Shared {
            registrations: Mutex::default(),
            spawn,
            report,
        }),
        owner: Arc::new(()),
    })
}
impl<E: ExtensionExecution> Clone for EventBus<E> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
            owner: self.owner.clone(),
        }
    }
}
impl<E: ExtensionExecution> Clone for EventBusController<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<E: ExtensionExecution> EventBus<E> {
    /// Runs listener prefixes without awaiting their tails, then returns a ready future.
    pub fn emit<'a>(
        &'a self,
        channel: &'a str,
        data: Value,
    ) -> impl Future<Output = ()> + Send + 'a {
        let data = Arc::new(data);
        for registration in self.snapshot(channel) {
            self.deliver(&registration, Arc::clone(&data));
        }
        std::future::ready(())
    }
    /// Adds a distinct registration on a literal channel.
    pub fn on(&self, channel: &str, handler: EventListener<E>) -> Subscription<E> {
        let registration = E::share(Registration {
            channel: channel.to_owned(),
            owner: Arc::clone(&self.owner),
            handler,
        });
        let subscription = Subscription {
            shared: self.shared.clone(),
            registration: E::downgrade(&registration),
        };
        lock(&self.shared.registrations).push(registration);
        subscription
    }
    /// Reports a synchronous callback failure through this bus's diagnostic destination.
    pub fn report_error(&self, channel: &str, error: &(dyn Error + Send + Sync)) {
        self.shared.report_error(channel, error);
    }
    /// Retains this emission's ordered listener collection.
    pub(crate) fn snapshot(&self, channel: &str) -> Vec<E::Shared<Registration<E>>> {
        lock(&self.shared.registrations)
            .iter()
            .filter(|registration| registration.channel == channel)
            .cloned()
            .collect()
    }
    /// Invokes one retained prefix under its subscribing identity.
    pub(crate) fn deliver(&self, registration: &Registration<E>, data: Arc<Value>) {
        let events = Self {
            shared: self.shared.clone(),
            owner: Arc::clone(&registration.owner),
        };
        let result = events.invoke_callback(|emitter| (registration.handler)(data, emitter));
        self.settle_listener(&registration.channel, result);
    }
    /// Applies the original report and continuation policy after a listener prefix returns.
    pub(crate) fn settle_listener(
        &self,
        channel: &str,
        result: Result<Option<EventTail<E>>, EventError>,
    ) {
        match result {
            Ok(Some(tail)) => {
                (self.shared.spawn)(E::tail_task(tail, channel.to_owned(), self.clone()));
            }
            Ok(None) => {}
            Err(error) => self.shared.report_error(channel, &*error),
        }
    }
}
impl<E: ExtensionExecution> Shared<E> {
    /// Renders the authored diagnostic without engine-specific formatting.
    fn report_error(&self, channel: &str, error: &(dyn Error + Send + Sync)) {
        (self.report)(&format!("Event handler error ({channel}): {error}\n"));
    }
}
impl<E: ExtensionExecution> EventBusController<E> {
    /// Creates another independent extension identity sharing this bus.
    #[must_use]
    pub fn for_extension(&self) -> EventBus<E> {
        EventBus {
            shared: self.0.shared.clone(),
            owner: Arc::new(()),
        }
    }
    /// Removes all registrations without cancelling their started tails.
    pub fn clear(&self) {
        let removed = std::mem::take(&mut *lock(&self.0.shared.registrations));
        drop(removed);
    }
}
impl<E: ExtensionExecution> Deref for EventBusController<E> {
    type Target = EventBus<E>;
    fn deref(&self) -> &EventBus<E> {
        &self.0
    }
}
impl<E: ExtensionExecution> Subscription<E> {
    /// Removes only this registration; repeated calls are inert.
    pub fn unsubscribe(&self) {
        let Some(target) = E::upgrade(&self.registration) else {
            return;
        };
        let removed = {
            let mut registrations = lock(&self.shared.registrations);
            registrations
                .iter()
                .position(|registration| E::ptr_eq(registration, &target))
                .map(|index| registrations.remove(index))
        };
        drop((removed, target));
    }
}

/// Settles one retained continuation, keeping its bus alive until it finishes.
pub(crate) async fn settle_tail<E: ExtensionExecution>(
    tail: EventTail<E>,
    channel: String,
    events: EventBus<E>,
) {
    if let Err(error) = tail.await {
        events.shared.report_error(&channel, &*error);
    }
}
