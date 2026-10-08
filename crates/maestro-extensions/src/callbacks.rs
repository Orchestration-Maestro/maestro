//! Borrowed emission within a synchronous extension callback.
use crate::event_bus::{EventBus, Registration};
use serde_json::Value;
use std::{cell::RefCell, sync::Arc};

/// A retained foreign listener and its immutable emission payload.
struct Delivery {
    /// Emission-time listener snapshot entry.
    registration: Arc<Registration>,
    /// Shared payload retained until callback return.
    data: Arc<Value>,
}
/// An emission capability borrowed for one synchronous callback.
pub struct CallbackEmitter<'a> {
    /// Extension identity invoking this callback.
    events: &'a EventBus,
    /// Only this invocation's deferred foreign deliveries.
    deferred: &'a RefCell<Vec<Delivery>>,
}
impl EventBus {
    /// Returns the callback's result after delivering its queued foreign listeners.
    ///
    /// Nested invocations have independent return boundaries, including errors
    /// returned as values. Panic recovery is not provided.
    pub fn invoke_callback<R>(&self, callback: impl for<'a> FnOnce(CallbackEmitter<'a>) -> R) -> R {
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
impl CallbackEmitter<'_> {
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
    pub fn events(&self) -> EventBus {
        self.events.clone()
    }
}
