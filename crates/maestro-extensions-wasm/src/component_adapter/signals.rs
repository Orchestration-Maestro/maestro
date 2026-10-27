//! Host cancellation signals behind the author signal port.

use std::rc::Rc;

use super::callbacks::{self, Identity};
use super::event_bus::{Emitter, subscription};
use super::{Exports, Imports};
use crate::types::{AbortListener, AbortSignal, ExtensionResult, SignalPort};
use crate::{CallbackEmitter, Subscription};

/// Host signal behind the facade.
pub(super) struct Signal<I: Imports> {
    /// The host the signal belongs to.
    imports: I,
    /// The host's cancellation flag.
    pub(super) signal: I::Signal,
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

/// Original host cancellation, or a mirror for a caller-defined signal port.
pub(super) enum Binding<I: Imports> {
    /// Keeps the exact resource the host supplied without subscribing to it again.
    Original(Rc<Signal<I>>),
    /// Follows an author-defined port through one temporary host signal.
    Mirrored(Mirror<I>),
}

impl<I: Imports> Binding<I> {
    /// Borrows an original host signal when available, otherwise follows its port.
    pub(super) fn new(imports: &I, caller: Option<&AbortSignal>) -> ExtensionResult<Self> {
        if let Some(original) = caller.and_then(AbortSignal::host::<Signal<I>>) {
            return Ok(Self::Original(original));
        }
        Mirror::new(imports, caller).map(Self::Mirrored)
    }

    /// The signal resource this native operation receives.
    pub(super) fn signal(&self) -> Option<&I::Signal> {
        match self {
            Self::Original(original) => Some(&original.signal),
            Self::Mirrored(mirror) => mirror.signal(),
        }
    }

    /// Removes only a mirror's observer after this operation settles.
    pub(super) fn settle<T>(self, result: ExtensionResult<T>) -> ExtensionResult<T> {
        match self {
            Self::Original(_) => result,
            Self::Mirrored(mirror) => mirror.settle(result),
        }
    }
}

/// A caller's signal listener, removed when its host call settles or is dropped.
pub(super) struct Listening(pub(super) Option<Subscription>);

impl Listening {
    /// Removes the listener once, returning the host's rejection.
    fn remove(&mut self) -> ExtensionResult<()> {
        self.0
            .take()
            .map_or(Ok(()), |subscription| subscription.unsubscribe())
    }
}

impl Drop for Listening {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

/// A host signal that follows a caller's signal for one host call.
pub(super) struct Mirror<I: Imports> {
    /// The host signal the call receives; absent without a caller signal.
    native: Option<Rc<I::Signal>>,
    /// The caller-signal listener that aborts it.
    listening: Listening,
    /// Keep arbitrary caller-local signal storage alive until its listener is removed.
    _caller: Option<AbortSignal>,
}

impl<I: Imports> Mirror<I> {
    /// Follows `caller` with a new host signal, aborted at once when the caller already is.
    pub(super) fn new(imports: &I, caller: Option<&AbortSignal>) -> ExtensionResult<Self> {
        let native = caller
            .map(|_| imports.new_signal())
            .transpose()?
            .map(Rc::new);
        let listening = Listening(
            caller
                .zip(native.as_ref())
                .map(|(caller, native)| {
                    let (imports, native) = (imports.clone(), Rc::clone(native));
                    caller.subscribe(Rc::new(move |_| imports.signal_abort(&native)))
                })
                .transpose()?,
        );
        let mirror = Self {
            native,
            listening,
            _caller: caller.cloned(),
        };
        if caller.is_some_and(AbortSignal::aborted)
            && let Some(native) = mirror.signal()
        {
            imports.signal_abort(native)?;
        }
        Ok(mirror)
    }

    /// The host signal the call receives.
    pub(super) fn signal(&self) -> Option<&I::Signal> {
        self.native.as_deref()
    }

    /// Stops following the caller once the call settled with `result`; the call's own error
    /// comes before a rejected listener removal.
    pub(super) fn settle<T>(mut self, result: ExtensionResult<T>) -> ExtensionResult<T> {
        let removed = self.listening.remove();
        result.and_then(|value| removed.map(|()| value))
    }
}

/// Wraps a host signal.
pub(crate) fn signal<I: Imports>(imports: &I, signal: I::Signal) -> AbortSignal {
    AbortSignal::supplied(Rc::new(Signal {
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

    /// Runs the terminal input listener under this identity on one chunk, with its borrowed
    /// emitter.
    pub(crate) fn invoke_terminal_input(
        &self,
        handler: u32,
        data: &str,
        emitter: &I::Emitter,
    ) -> ExtensionResult<Option<crate::TerminalInputResult>> {
        let listener = callbacks::find::<crate::TerminalInputHandler>(handler)?;
        let port = Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        listener(data, CallbackEmitter::new(&port))
    }
}
