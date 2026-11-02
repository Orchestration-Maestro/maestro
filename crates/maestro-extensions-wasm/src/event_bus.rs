//! Host-backed channels, explicit subscriptions and callback-scoped emission.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{ExtensionFuture, ExtensionResult};
use serde_json::Value;
use std::rc::Rc;

/// Host operations behind an event bus.
pub trait EventBusPort {
    /// Emits through the supplied host.
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()>;
    /// Registers a listener.
    ///
    /// # Errors
    /// Returns the host rejection.
    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription>;
}
/// Host operation behind an explicit remover.
pub trait SubscriptionPort {
    /// Removes the subscription.
    ///
    /// # Errors
    /// Returns the host rejection.
    fn unsubscribe(&self) -> ExtensionResult<()>;
}
/// Emission available during a synchronous callback.
pub trait CallbackEmitterPort {
    /// Emits through this callback's capability.
    ///
    /// # Errors
    /// Returns the host rejection.
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()>;
}
/// Synchronous prefix returning an optional independently owned continuation.
pub type EventListener = Rc<
    dyn for<'a> Fn(
        Value,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<ExtensionFuture<'static, ()>>>,
>;
/// A shared handle to the supplied bus port.
#[derive(Clone)]
pub struct EventBus(Rc<dyn EventBusPort>);
impl EventBus {
    /// Wraps an alternate host adapter.
    #[must_use]
    pub fn new(port: Rc<dyn EventBusPort>) -> Self {
        Self(port)
    }
    /// Forwards an ordinary asynchronous emission.
    #[must_use]
    pub fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        self.0.emit(channel, data)
    }
    /// Registers a prefix callback.
    ///
    /// # Errors
    /// Returns the host rejection.
    pub fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        self.0.on(channel, listener)
    }
}
/// An explicitly invoked remover; dropping this facade does not unsubscribe.
#[derive(Clone)]
pub struct Subscription(Rc<dyn SubscriptionPort>);
impl Subscription {
    /// Wraps an alternate remover adapter.
    #[must_use]
    pub fn new(port: Rc<dyn SubscriptionPort>) -> Self {
        Self(port)
    }
    /// Forwards explicit removal.
    ///
    /// # Errors
    /// Returns the host rejection.
    pub fn unsubscribe(&self) -> ExtensionResult<()> {
        self.0.unsubscribe()
    }
}
/// A capability borrowed only for a synchronous callback.
///
/// `callback_emitter_cannot_escape_into_tail`:
/// ```compile_fail
/// use maestro_extensions_wasm::{CallbackEmitter, ExtensionFuture};
/// fn escape(emitter: CallbackEmitter<'_>) -> ExtensionFuture<'static, ()> {
///     Box::pin(async move { emitter.emit("later", serde_json::Value::Null) })
/// }
/// ```
#[derive(Clone, Copy)]
pub struct CallbackEmitter<'a>(&'a dyn CallbackEmitterPort);
impl<'a> CallbackEmitter<'a> {
    /// Borrows an alternate callback adapter.
    #[must_use]
    pub fn new(port: &'a dyn CallbackEmitterPort) -> Self {
        Self(port)
    }
    /// Forwards emission while the callback capability is borrowed.
    ///
    /// # Errors
    /// Returns the host rejection.
    pub fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.0.emit(channel, data)
    }
}
