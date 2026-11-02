//! Borrowed emission within a synchronous extension callback.
use crate::{
    EventError, ExtensionExecution,
    event_bus::{EventBus, Registration},
};
use maestro_models::Threaded;
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

/// A retained foreign listener and its immutable emission payload.
struct Delivery<E: ExtensionExecution> {
    /// Emission-time listener snapshot entry.
    registration: E::Shared<Registration<E>>,
    /// Shared payload retained until callback return.
    data: Arc<Value>,
}

/// One invocation's original bus identity and deferred foreign deliveries.
struct FrameState<E: ExtensionExecution> {
    /// Extension identity invoking this callback.
    events: EventBus<E>,
    /// Only this invocation's deferred foreign deliveries.
    deferred: RefCell<Vec<Delivery<E>>>,
    /// Retained capabilities cannot emit after the invoking callback returns.
    active: Cell<bool>,
}

/// Retained emission capability valid only until its synchronous callback returns.
///
/// Copies share the original invocation's bus and deferred queue. Unlike the
/// ordinary owned bus returned by [`CallbackEmitter::events`], this capability
/// rejects emission once that invocation has ended.
pub struct CallbackFrame<E: ExtensionExecution = Threaded>(Rc<FrameState<E>>);

impl<E: ExtensionExecution> Clone for CallbackFrame<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: ExtensionExecution> CallbackFrame<E> {
    /// Runs same-extension listeners now and queues foreign listeners until return.
    ///
    /// # Errors
    /// Returns the native stale-context error once this callback has returned.
    pub fn emit(&self, channel: &str, data: Value) -> Result<(), EventError> {
        if !self.0.active.get() {
            return Err(crate::loader::STALE_MESSAGE.into());
        }
        self.deliver(channel, data);
        Ok(())
    }

    /// Retains the emission-time listener order for a split component walk.
    ///
    /// # Errors
    /// Returns the native stale-context error after this callback returns.
    pub fn snapshot(
        &self,
        channel: &str,
        data: Value,
    ) -> Result<Vec<CallbackDelivery<E>>, EventError> {
        if !self.0.active.get() {
            return Err(crate::loader::STALE_MESSAGE.into());
        }
        let data = Arc::new(data);
        Ok(self
            .0
            .events
            .snapshot(channel)
            .into_iter()
            .map(|registration| CallbackDelivery {
                frame: self.clone(),
                registration,
                data: data.clone(),
            })
            .collect())
    }

    /// Delivers after the caller establishes that this invocation is live.
    fn deliver(&self, channel: &str, data: Value) {
        let data = Arc::new(data);
        for registration in self.0.events.snapshot(channel) {
            CallbackDelivery {
                frame: self.clone(),
                registration,
                data: data.clone(),
            }
            .deliver_active();
        }
    }

    /// Retires all retained aliases before releasing foreign listeners outside borrows.
    fn retire(&self) -> Vec<Delivery<E>> {
        self.0.active.set(false);
        std::mem::take(&mut *self.0.deferred.borrow_mut())
    }
}

/// The callback's lexical owner also retires retained aliases during unwinding.
struct Invocation<E: ExtensionExecution>(CallbackFrame<E>);

impl<E: ExtensionExecution> Invocation<E> {
    /// Creates the original independent return boundary for one callback.
    fn new(events: EventBus<E>) -> Self {
        Self(CallbackFrame(Rc::new(FrameState {
            events,
            deferred: RefCell::default(),
            active: Cell::new(true),
        })))
    }
    /// Retires aliases before delivering this callback's foreign listeners.
    fn finish(self) {
        for delivery in self.0.retire() {
            self.0
                .0
                .events
                .deliver(&delivery.registration, delivery.data);
        }
    }
}

impl<E: ExtensionExecution> Drop for Invocation<E> {
    fn drop(&mut self) {
        drop(self.0.retire());
    }
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
    /// Borrowing the lexical owner prevents native use after its return boundary.
    frame: &'a CallbackFrame<E>,
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
        let invocation = Invocation::new(self.clone());
        let result = callback(CallbackEmitter {
            frame: &invocation.0,
        });
        invocation.finish();
        result
    }
}

impl<E: ExtensionExecution> CallbackEmitter<'_, E> {
    /// Runs same-extension listeners now and queues foreign listeners until return.
    pub fn emit(&self, channel: &str, data: Value) {
        self.frame.deliver(channel, data);
    }

    /// Supplies an owned ordinary emitter with this callback's identity.
    #[must_use]
    pub fn events(&self) -> EventBus<E> {
        self.frame.0.events.clone()
    }

    /// Retains this invocation's original emission frame without extending its validity.
    #[must_use]
    pub fn frame(&self) -> CallbackFrame<E> {
        self.frame.clone()
    }
}

/// One retained native listener from an emission-time snapshot.
pub struct CallbackDelivery<E: ExtensionExecution = Threaded> {
    /// Original live callback boundary.
    frame: CallbackFrame<E>,
    /// Listener kept alive even if it is removed during emission.
    registration: E::Shared<Registration<E>>,
    /// Original immutable event payload.
    data: Arc<Value>,
}
impl<E: ExtensionExecution> CallbackDelivery<E> {
    /// The registered callback identity used by a component's local listener lookup.
    pub fn listener(&self) -> &crate::EventListener<E> {
        &self.registration.handler
    }
    /// Opens an independent native return boundary for a locally executed prefix.
    ///
    /// # Errors
    /// Returns the native stale-context error after the emitting callback returns.
    pub fn begin(self) -> Result<ListenerInvocation<E>, EventError> {
        self.check_active()?;
        let events = EventBus {
            shared: self.frame.0.events.shared.clone(),
            owner: self.registration.owner.clone(),
        };
        Ok(ListenerInvocation {
            invocation: Invocation::new(events),
            delivery: self,
        })
    }
    /// Uses the original immediate-versus-foreign callback policy.
    ///
    /// # Errors
    /// Returns the native stale-context error after the emitting callback returns.
    pub fn deliver(self) -> Result<(), EventError> {
        self.check_active()?;
        self.deliver_active();
        Ok(())
    }
    /// Rejects operations through snapshots retained past their original callback.
    fn check_active(&self) -> Result<(), EventError> {
        if self.frame.0.active.get() {
            Ok(())
        } else {
            Err(crate::loader::STALE_MESSAGE.into())
        }
    }
    /// Applies native delivery policy after the caller established frame validity.
    fn deliver_active(self) {
        if Arc::ptr_eq(&self.registration.owner, &self.frame.0.events.owner) {
            self.frame.0.events.deliver(&self.registration, self.data);
        } else {
            self.frame.0.deferred.borrow_mut().push(Delivery {
                registration: self.registration,
                data: self.data,
            });
        }
    }
}

/// A locally executed listener's native frame, error destination and tail owner.
pub struct ListenerInvocation<E: ExtensionExecution = Threaded> {
    /// Retires aliases and foreign deliveries at this listener's return boundary.
    invocation: Invocation<E>,
    /// Original registration stays alive through the local prefix.
    delivery: CallbackDelivery<E>,
}
impl<E: ExtensionExecution> ListenerInvocation<E> {
    /// Retains this listener's independent frame without extending its validity.
    #[must_use]
    pub fn frame(&self) -> CallbackFrame<E> {
        self.invocation.0.clone()
    }
    /// Completes the same native error, tail and foreign-delivery path as a host prefix.
    pub fn finish(self, result: Result<Option<crate::EventTail<E>>, EventError>) {
        self.invocation.finish();
        self.delivery
            .frame
            .0
            .events
            .settle_listener(&self.delivery.registration.channel, result);
    }
}
