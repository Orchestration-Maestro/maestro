//! Host cancellation signals behind the author signal port.

use std::rc::Rc;

use super::callbacks::{self, Identity};
use super::event_bus::{Emitter, subscription};
use super::{Exports, Imports};
use crate::types::{AbortListener, AbortSignal, ExtensionResult, SignalPort};
use crate::{CallbackEmitter, Subscription};

/// Host signal behind the facade.
struct Signal<I: Imports> {
    /// The host the signal belongs to.
    imports: I,
    /// The host's cancellation flag.
    signal: I::Signal,
}

impl<I: Imports> SignalPort for Signal<I> {
    fn aborted(&self) -> bool {
        self.imports.aborted(&self.signal)
    }

    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let identity = Identity::new(&self.imports);
        let handle = self
            .imports
            .signal_subscribe(&self.signal, &identity.handle)?;
        identity.keep(listener);
        Ok(subscription(self.imports.clone(), handle))
    }
}

/// Wraps a host signal.
pub(crate) fn signal<I: Imports>(imports: &I, signal: I::Signal) -> AbortSignal {
    AbortSignal::new(Rc::new(Signal {
        imports: imports.clone(),
        signal,
    }))
}

impl<I: Imports> Exports<I> {
    /// Runs the cancellation listener under this identity once, with its borrowed emitter.
    pub(crate) fn invoke_abort(&self, handler: u32, emitter: &I::Emitter) -> ExtensionResult<()> {
        let listener = callbacks::take::<AbortListener>(handler)?;
        let port = Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        listener(CallbackEmitter::new(&port))
    }
}

/// A caller's arbitrary signal forwarded to one native cancellation.
pub(super) struct Binding<I: Imports> {
    /// Independently owned native flag retained by its caller and abort observer.
    pub(super) native: Rc<I::Signal>,
    /// Removed when the containing invocation releases its controls.
    listening: Option<crate::Subscription>,
}
impl<I: Imports> Binding<I> {
    /// Subscribe before testing abortion, so an intervening abort is not lost.
    pub(super) fn new(imports: &I, caller: &AbortSignal) -> ExtensionResult<Self> {
        let native = Rc::new(imports.new_signal()?);
        let (notify, target) = (imports.clone(), Rc::clone(&native));
        let binding = Self {
            native,
            listening: Some(caller.subscribe(Rc::new(move |_| notify.signal_abort(&target)))?),
        };
        if caller.aborted() {
            imports.signal_abort(&binding.native)?;
        }
        Ok(binding)
    }

    /// Remove the original caller's listener once, preserving explicit removal errors.
    pub(super) fn remove(&mut self) -> ExtensionResult<()> {
        self.listening
            .take()
            .map_or(Ok(()), |listening| listening.unsubscribe())
    }
}
impl<I: Imports> Drop for Binding<I> {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}
