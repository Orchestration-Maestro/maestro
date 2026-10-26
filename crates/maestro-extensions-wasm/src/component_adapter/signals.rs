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
