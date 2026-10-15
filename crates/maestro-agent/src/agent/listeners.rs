//! Identity-deduplicated live subscription traversal.
use super::{Agent, CustomAgentMessages, lock};
use crate::AgentEvent;
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock, Weak},
};
#[cfg(not(target_arch = "wasm32"))]
/// Awaited listener receiving the shared mutable event after state reduction.
pub type AgentListener<C = crate::NoCustomMessages> = Arc<
    dyn Fn(Arc<RwLock<AgentEvent<C>>>, Cancellation) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
#[cfg(target_arch = "wasm32")]
/// Awaited listener receiving the shared mutable event after state reduction.
pub type AgentListener<C = crate::NoCustomMessages> = Arc<
    dyn Fn(Arc<RwLock<AgentEvent<C>>>, Cancellation) -> BoxFuture<Result<(), DiagnosticErrorInfo>>,
>;
/// Live callbacks indexed by monotonically increasing registration order.
pub(super) struct Listeners<C: CustomAgentMessages> {
    /// Next registration identity.
    next: u64,
    /// Current registrations.
    entries: BTreeMap<u64, AgentListener<C>>,
}
impl<C: CustomAgentMessages> Default for Listeners<C> {
    fn default() -> Self {
        Self {
            next: 0,
            entries: BTreeMap::new(),
        }
    }
}
/// Explicit removal handle; dropping it leaves the listener registered.
pub struct Subscription<C: CustomAgentMessages = crate::NoCustomMessages> {
    /// Non-owning registry owner.
    agent: Weak<Agent<C>>,
    /// Callback identity, including subsequent re-registration.
    listener: Weak<ListenerObject<C>>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Target-specific callback object.
type ListenerObject<C> = dyn Fn(Arc<RwLock<AgentEvent<C>>>, Cancellation) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
    + Send
    + Sync;
#[cfg(target_arch = "wasm32")]
/// Target-specific callback object.
type ListenerObject<C> =
    dyn Fn(Arc<RwLock<AgentEvent<C>>>, Cancellation) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;
impl<C: CustomAgentMessages> Agent<C> {
    /// Register callback identity once; additions are visible to an active traversal.
    pub fn subscribe(self: &Arc<Self>, listener: AgentListener<C>) -> Subscription<C> {
        let subscription = Subscription {
            agent: Arc::downgrade(self),
            listener: Arc::downgrade(&listener),
        };
        let mut listeners = lock(&self.listeners);
        if !listeners
            .entries
            .values()
            .any(|old| Arc::ptr_eq(old, &listener))
        {
            let id = listeners.next;
            listeners.next += 1;
            listeners.entries.insert(id, listener);
        }
        subscription
    }
}
impl<C: CustomAgentMessages> Subscription<C> {
    /// Remove this callback identity, including a later re-addition, idempotently.
    pub fn unsubscribe(&self) {
        let removed = self.agent.upgrade().and_then(|agent| {
            let mut listeners = lock(&agent.listeners);
            let key = listeners.entries.iter().find_map(|(key, value)| {
                std::ptr::addr_eq(Arc::as_ptr(value), self.listener.as_ptr()).then_some(*key)
            });
            key.and_then(|key| listeners.entries.remove(&key))
        });
        drop(removed);
    }
}
impl<C: CustomAgentMessages + 'static> Agent<C> {
    /// Reduce and invoke the first listener now; await live successors without guards.
    pub(super) fn emit(
        self: &Arc<Self>,
        event: AgentEvent<C>,
        signal: Cancellation,
    ) -> impl std::future::Future<Output = Result<(), DiagnosticErrorInfo>> + use<C> {
        self.reduce(&event);
        let event = Arc::new(RwLock::new(event));
        let mut next = 0;
        let agent = Arc::clone(self);
        let mut invoke_next = move || {
            let listener = lock(&agent.listeners)
                .entries
                .range(next..)
                .next()
                .map(|(id, listener)| (*id, Arc::clone(listener)));
            listener.map(|(id, listener)| {
                next = id + 1;
                listener(Arc::clone(&event), signal.clone())
            })
        };
        let mut pending = invoke_next();
        async move {
            while let Some(future) = pending {
                future.await?;
                pending = invoke_next();
            }
            Ok(())
        }
    }
}
