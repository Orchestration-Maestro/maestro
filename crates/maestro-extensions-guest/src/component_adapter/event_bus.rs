//! Event operations forwarded without a guest delivery scheduler.
use super::{Exports, Imports, callbacks};
use crate::{
    CallbackEmitter, CallbackEmitterPort, EventBus, EventBusPort, EventListener, ExtensionFuture,
    ExtensionResult, Subscription, SubscriptionPort,
};
use serde_json::Value;
use std::rc::Rc;
/// Host imports behind a bus facade.
struct Bus<I>(I);
/// Owned host remover.
struct Remover<I: Imports> {
    /// Host imports.
    imports: I,
    /// Subscription resource.
    handle: I::Subscription,
}
/// Borrowed emitter for one invocation.
pub(super) struct Emitter<'a, I: Imports> {
    /// Host imports.
    pub(super) imports: &'a I,
    /// Borrowed resource.
    pub(super) handle: &'a I::Emitter,
}
impl<I: Imports> EventBusPort for Bus<I> {
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .event_emit(channel.to_owned(), data.to_string())
                .await
        })
    }
    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        let identity = callbacks::Identity::new(&self.0);
        let handle = self.0.event_on(channel, &identity.handle)?;
        identity.keep(listener);
        Ok(subscription(self.0.clone(), handle))
    }
}
/// Wraps a host remover in the author facade.
pub(super) fn subscription<I: Imports>(imports: I, handle: I::Subscription) -> Subscription {
    Subscription::new(Rc::new(Remover { imports, handle }))
}
impl<I: Imports> SubscriptionPort for Remover<I> {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        self.imports.unsubscribe(&self.handle)
    }
}
impl<I: Imports> CallbackEmitterPort for Emitter<'_, I> {
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.imports
            .callback_emit(self.handle, channel, &data.to_string())
    }
}
/// Creates a facade bound to these host imports.
pub(crate) fn events<I: Imports>(imports: I) -> EventBus {
    EventBus::new(Rc::new(Bus(imports)))
}
impl<I: Imports> Exports<I> {
    /// Runs the synchronous prefix and announces its independently owned tail.
    pub(crate) fn invoke_listener(
        &self,
        handler: u32,
        data: &str,
        emitter: &I::Emitter,
    ) -> ExtensionResult<()> {
        let listener = callbacks::find::<EventListener>(handler)?;
        let data = serde_json::from_str(data).map_err(|error| error.to_string())?;
        let port = Emitter {
            imports: &self.imports,
            handle: emitter,
        };
        if let Some(tail) = listener(data, CallbackEmitter::new(&port))? {
            let identity = callbacks::Identity::new(&self.imports);
            self.imports.start_event_tail(emitter, &identity.handle)?;
            identity.keep(tail);
        }
        Ok(())
    }
    /// Consumes and awaits the tail under this identity.
    pub(crate) async fn invoke_tail(handler: u32) -> ExtensionResult<()> {
        callbacks::take::<ExtensionFuture<'static, ()>>(handler)?.await
    }
}
