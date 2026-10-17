//! Borrowed emission within a synchronous extension callback.
use crate::{
    ExtensionExecution,
    event_bus::{EventBus, Registration},
};
use maestro_models::Threaded;
use serde_json::Value;
use std::{cell::RefCell, sync::Arc};

/// A retained foreign listener and its immutable emission payload.
struct Delivery<E: ExtensionExecution> {
    /// Emission-time listener snapshot entry.
    registration: E::Shared<Registration<E>>,
    /// Shared payload retained until callback return.
    data: Arc<Value>,
}
/// An emission capability borrowed for one synchronous callback.
///
/// The emitter cannot outlive its invocation; use [`CallbackEmitter::events`]
/// for an owned emitter that a tail may keep:
///
/// ```compile_fail,E0521
/// use maestro_extensions::create_event_bus;
/// use std::sync::Arc;
///
/// fn callback_emitter_cannot_outlive_invocation() {
///     let bus = create_event_bus(Arc::new(|_| {}), Arc::new(|_| {}));
///     let mut escaped = None;
///     bus.invoke_callback(|emitter| escaped = Some(emitter));
///     drop(escaped);
/// }
/// ```
pub struct CallbackEmitter<'a, E: ExtensionExecution = Threaded> {
    /// Extension identity invoking this callback.
    events: &'a EventBus<E>,
    /// Only this invocation's deferred foreign deliveries.
    deferred: &'a RefCell<Vec<Delivery<E>>>,
}
impl<E: ExtensionExecution> EventBus<E> {
    /// Returns the callback's result after delivering its queued foreign listeners.
    ///
    /// Nested invocations have independent return boundaries, including errors
    /// returned as values. Panic recovery is not provided.
    pub fn invoke_callback<R>(
        &self,
        callback: impl for<'a> FnOnce(CallbackEmitter<'a, E>) -> R,
    ) -> R {
        let deferred = RefCell::new(Vec::new());
        let result = callback(CallbackEmitter {
            events: self,
            deferred: &deferred,
        });
        for delivery in deferred.into_inner() {
            self.deliver(&delivery.registration, delivery.data);
        }
        result
    }
}
impl<E: ExtensionExecution> CallbackEmitter<'_, E> {
    /// Runs same-extension listeners now and queues foreign listeners until return.
    pub fn emit(&self, channel: &str, data: Value) {
        let data = Arc::new(data);
        for registration in self.events.snapshot(channel) {
            if Arc::ptr_eq(&registration.owner, &self.events.owner) {
                self.events.deliver(&registration, Arc::clone(&data));
            } else {
                self.deferred.borrow_mut().push(Delivery {
                    registration,
                    data: Arc::clone(&data),
                });
            }
        }
    }
    /// Supplies an owned ordinary emitter with this callback's identity.
    #[must_use]
    pub fn events(&self) -> EventBus<E> {
        self.events.clone()
    }
}
