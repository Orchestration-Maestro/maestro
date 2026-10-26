//! Typed forwarding to the host's event buffer and independent result owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult};
use std::rc::Rc;

/// Operations supplied by an event-stream adapter.
pub trait EventStreamPort<T, R> {
    /// Admit an event, running completion callbacks synchronously.
    ///
    /// # Errors
    /// Returns the host's admission failure.
    fn push(&self, event: T) -> ExtensionResult<()>;
    /// Stop admission and optionally publish the first result.
    ///
    /// # Errors
    /// Returns the host's failure.
    fn end(&self, result: Option<R>) -> ExtensionResult<()>;
    /// Observe one event, competing with other readers.
    fn next(&self) -> ExtensionFuture<'static, Option<Rc<T>>>;
    /// Observe the retained result without consuming events.
    fn result(&self) -> ExtensionFuture<'static, Rc<R>>;
}

/// A host-buffered stream retaining arbitrary caller-local events and results.
pub struct EventStream<T, R = T>(Rc<dyn EventStreamPort<T, R>>);
impl<T, R> Clone for EventStream<T, R> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: 'static, R: 'static> EventStream<T, R> {
    /// Wrap an adapter without introducing a second event buffer.
    #[must_use]
    pub fn from_port(port: Rc<dyn EventStreamPort<T, R>>) -> Self {
        Self(port)
    }

    /// Create a host-backed stream with synchronous local completion callbacks.
    ///
    /// # Errors
    /// Returns the host's creation failure.
    #[cfg(target_arch = "wasm32")]
    pub fn new(
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> ExtensionResult<Self> {
        crate::component_adapter::new_event_stream(is_complete, extract_result)
    }

    /// Admit an event; late events do not run completion callbacks.
    ///
    /// # Errors
    /// Returns the host's admission failure.
    pub fn push(&self, event: T) -> ExtensionResult<()> {
        self.0.push(event)
    }
    /// End iteration without replacing a published result or discarding queued events.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn end(&self, result: Option<R>) -> ExtensionResult<()> {
        self.0.end(result)
    }
    /// Read the next event or EOF; observers compete in registration order.
    #[must_use]
    pub fn next(&self) -> ExtensionFuture<'static, Option<Rc<T>>> {
        self.0.next()
    }
    /// Observe the first result independently of event iteration.
    #[must_use]
    pub fn result(&self) -> ExtensionFuture<'static, Rc<R>> {
        self.0.result()
    }
}
