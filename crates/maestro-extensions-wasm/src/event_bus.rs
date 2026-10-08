//! The shared event bus and the emitter synchronous callbacks borrow.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use serde_json::Value;

use crate::types::{ExtensionFuture, ExtensionResult};

/// Host capability behind a [`CallbackEmitter`].
pub trait CallbackEmitterPort {
    /// Emits `data` on `channel`; listeners in other extensions receive it right after the
    /// current callback returns.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the emission.
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()>;
}

/// Emitter a synchronous callback borrows for the duration of the call.
pub struct CallbackEmitter<'a>(&'a dyn CallbackEmitterPort);

impl<'a> CallbackEmitter<'a> {
    /// Borrows the host's emitter.
    #[must_use]
    pub fn new(port: &'a dyn CallbackEmitterPort) -> Self {
        Self(port)
    }

    /// Emits `data` on `channel`.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the emission.
    pub fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.0.emit(channel, data)
    }
}

/// Host capability behind a [`Subscription`].
pub trait SubscriptionPort {
    /// Stops delivery to the listener.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    fn unsubscribe(&self) -> ExtensionResult<()>;
}

/// A bus subscription. Dropping it does not unsubscribe; call [`Subscription::unsubscribe`].
pub struct Subscription(Rc<dyn SubscriptionPort>);

impl Subscription {
    /// Wraps the host's subscription.
    #[must_use]
    pub fn new(port: Rc<dyn SubscriptionPort>) -> Self {
        Self(port)
    }

    /// Stops delivery to the listener.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the request.
    pub fn unsubscribe(&self) -> ExtensionResult<()> {
        self.0.unsubscribe()
    }
}

/// Listener of a bus channel: its synchronous prefix returns an optional future for the
/// asynchronous tail the host runs later.
pub type EventListener = Rc<
    dyn for<'a> Fn(
        Value,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Option<ExtensionFuture<'static, ()>>>,
>;

/// Host capability behind an [`EventBus`].
pub trait EventBusPort {
    /// Emits `data` on `channel` and resolves once the host has delivered it.
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()>;

    /// Subscribes a listener to a channel.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the subscription.
    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription>;
}

/// The event bus shared by extensions.
#[derive(Clone)]
pub struct EventBus(Rc<dyn EventBusPort>);

impl EventBus {
    /// Wraps the host's bus.
    #[must_use]
    pub fn new(port: Rc<dyn EventBusPort>) -> Self {
        Self(port)
    }

    /// Emits `data` on `channel` and resolves once the host has delivered it.
    #[must_use]
    pub fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        self.0.emit(channel, data)
    }

    /// Subscribes a listener to a channel. The listener stays subscribed until
    /// [`Subscription::unsubscribe`], also when the returned value is dropped.
    ///
    /// # Errors
    /// Returns the host's message when it rejects the subscription.
    pub fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        self.0.on(channel, listener)
    }
}
