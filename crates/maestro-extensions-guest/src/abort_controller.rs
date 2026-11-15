//! Caller-owned cancellation for operations that accept an abort signal.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use crate::{
    AbortListener, AbortSignal, CallbackEmitter, CallbackEmitterPort, ExtensionResult, SignalPort,
    Subscription, SubscriptionPort,
};

/// Cancels operations sharing its signal. Clones share the same cancellation state.
#[derive(Clone, Default)]
pub struct AbortController(Rc<State>);

/// Cancellation state and listeners in registration order.
#[derive(Default)]
struct State {
    /// Whether cancellation has occurred.
    aborted: Cell<bool>,
    /// Live listeners, identified independently of their callbacks.
    listeners: RefCell<Vec<(usize, AbortListener)>>,
    /// Next subscription identity.
    next: Cell<usize>,
}

/// Removes one listener without keeping its controller alive.
struct Removal(Weak<State>, usize);

/// Caller-owned cancellation has no host event channel to emit through.
struct Silent;

impl CallbackEmitterPort for Silent {
    fn emit(&self, _channel: &str, _data: serde_json::Value) -> ExtensionResult<()> {
        Ok(())
    }
}

impl SubscriptionPort for Removal {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        if let Some(state) = self.0.upgrade() {
            let removed = {
                let mut listeners = state.listeners.borrow_mut();
                listeners
                    .iter()
                    .position(|(id, _)| *id == self.1)
                    .map(|index| listeners.remove(index))
            };
            drop(removed);
        }
        Ok(())
    }
}

/// A signal keeps its state alive independently of the controller.
struct Signal(Rc<State>);

impl SignalPort for Signal {
    fn aborted(&self) -> bool {
        self.0.aborted.get()
    }

    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let id = self.0.next.replace(self.0.next.get() + 1);
        if !self.aborted() {
            self.0.listeners.borrow_mut().push((id, listener));
        }
        Ok(Subscription::new(Rc::new(Removal(
            Rc::downgrade(&self.0),
            id,
        ))))
    }
}

impl AbortController {
    /// Creates a controller whose signal is not aborted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a signal sharing this controller's cancellation state.
    #[must_use]
    pub fn signal(&self) -> AbortSignal {
        AbortSignal::new(Rc::new(Signal(Rc::clone(&self.0))))
    }

    /// Aborts once, invoking the current listeners synchronously in registration order.
    ///
    /// Listeners added after cancellation are not invoked. Callback emission has no effect.
    ///
    /// # Errors
    /// Returns the first listener error and stops delivering that abort.
    pub fn abort(&self) -> ExtensionResult<()> {
        if self.0.aborted.replace(true) {
            return Ok(());
        }
        let listeners = self.0.listeners.replace(Vec::new());
        for (_, listener) in listeners {
            listener(CallbackEmitter::new(&Silent))?;
        }
        Ok(())
    }
}
