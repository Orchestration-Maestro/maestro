//! Ordered channels shared by independently identified extensions.
use crate::callbacks::CallbackEmitter;
use serde_json::Value;
use std::{
    error::Error,
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::{Arc, Mutex},
};

/// A native listener failure.
pub type EventError = Box<dyn Error + Send + Sync + 'static>;
/// An owned asynchronous listener continuation.
pub type EventTail = Pin<Box<dyn Future<Output = Result<(), EventError>> + Send + 'static>>;
/// Enqueues work on the emitting code's serial cooperative executor.
///
/// Must not poll inline or concurrently with the emitting synchronous segment.
/// A genuine suspension must allow queued work to progress.
pub type TailSpawner =
    Arc<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send + 'static>>) + Send + Sync + 'static>;
/// Receives a complete attributed diagnostic, including its final newline.
pub type ErrorReporter = Arc<dyn Fn(&str) + Send + Sync + 'static>;
/// Runs a synchronous prefix and optionally returns an owned continuation.
pub type EventListener = Arc<
    dyn for<'a> Fn(Arc<Value>, CallbackEmitter<'a>) -> Result<Option<EventTail>, EventError>
        + Send
        + Sync
        + 'static,
>;
/// One independently removable listener registration.
pub(crate) struct Registration {
    /// Literal channel name.
    channel: String,
    /// Identity of the subscribing extension.
    pub(crate) owner: Arc<()>,
    /// Caller-owned listener.
    handler: EventListener,
}
/// Shared subscriptions and caller-supplied execution facilities.
pub(crate) struct Shared {
    /// Ordered registrations, locked only for collection operations.
    registrations: Mutex<Vec<Arc<Registration>>>,
    /// Serial continuation enqueue facility.
    spawn: TailSpawner,
    /// Diagnostic destination.
    report: ErrorReporter,
}
/// An extension's shared channel capability.
#[derive(Clone)]
pub struct EventBus {
    /// Bus shared by all extension handles.
    pub(crate) shared: Arc<Shared>,
    /// Identity retained by cloned handles.
    pub(crate) owner: Arc<()>,
}
/// Owns the ability to clear all subscriptions on a shared bus.
#[derive(Clone)]
pub struct EventBusController(EventBus);
/// An explicit, idempotent remover; dropping it leaves the listener registered.
pub struct Subscription {
    /// Bus containing the registration.
    shared: Arc<Shared>,
    /// Registration identity without ownership of the listener.
    registration: std::sync::Weak<Registration>,
}
/// Creates an empty shared bus with caller-owned execution and diagnostics.
#[must_use]
pub fn create_event_bus(spawn: TailSpawner, report: ErrorReporter) -> EventBusController {
    EventBusController(EventBus {
        shared: Arc::new(Shared {
            registrations: Mutex::default(),
            spawn,
            report,
        }),
        owner: Arc::new(()),
    })
}
impl EventBus {
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
    pub fn on(&self, channel: &str, handler: EventListener) -> Subscription {
        let registration = Arc::new(Registration {
            channel: channel.to_owned(),
            owner: Arc::clone(&self.owner),
            handler,
        });
        let subscription = Subscription {
            shared: Arc::clone(&self.shared),
            registration: Arc::downgrade(&registration),
        };
        self.shared
            .registrations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(registration);
        subscription
    }
    /// Retains this emission's ordered listener collection.
    pub(crate) fn snapshot(&self, channel: &str) -> Vec<Arc<Registration>> {
        self.shared
            .registrations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|registration| registration.channel == channel)
            .cloned()
            .collect()
    }
    /// Invokes one retained prefix under its subscribing identity.
    pub(crate) fn deliver(&self, registration: &Registration, data: Arc<Value>) {
        let events = Self {
            shared: Arc::clone(&self.shared),
            owner: Arc::clone(&registration.owner),
        };
        let result = events.invoke_callback(|emitter| (registration.handler)(data, emitter));
        match result {
            Ok(Some(tail)) => self.shared.start_tail(registration.channel.clone(), tail),
            Ok(None) => {}
            Err(error) => self.shared.report_error(&registration.channel, &*error),
        }
    }
}
impl Shared {
    /// Owns tail execution independently of its removed registration.
    fn start_tail(self: &Arc<Self>, channel: String, tail: EventTail) {
        let shared = Arc::clone(self);
        (self.spawn)(Box::pin(async move {
            if let Err(error) = tail.await {
                shared.report_error(&channel, &*error);
            }
        }));
    }
    /// Renders the authored diagnostic without engine-specific formatting.
    fn report_error(&self, channel: &str, error: &(dyn Error + Send + Sync)) {
        (self.report)(&format!("Event handler error ({channel}): {error}\n"));
    }
}
impl EventBusController {
    /// Creates another independent extension identity sharing this bus.
    #[must_use]
    pub fn for_extension(&self) -> EventBus {
        EventBus {
            shared: Arc::clone(&self.0.shared),
            owner: Arc::new(()),
        }
    }
    /// Removes all registrations without cancelling their started tails.
    pub fn clear(&self) {
        let removed = std::mem::take(
            &mut *self
                .0
                .shared
                .registrations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        drop(removed);
    }
}
impl Deref for EventBusController {
    type Target = EventBus;
    fn deref(&self) -> &EventBus {
        &self.0
    }
}
impl Subscription {
    /// Removes only this registration; repeated calls are inert.
    pub fn unsubscribe(&self) {
        let removed = {
            let mut registrations = self
                .shared
                .registrations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            registrations
                .iter()
                .position(|registration| {
                    std::ptr::eq(Arc::as_ptr(registration), self.registration.as_ptr())
                })
                .map(|index| registrations.remove(index))
        };
        drop(removed);
    }
}
