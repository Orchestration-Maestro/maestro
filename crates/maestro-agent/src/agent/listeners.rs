//! Identity-deduplicated live subscription traversal.
use super::{Agent, CustomAgentMessages, lock};
use crate::AgentEvent;
use crate::types::{AgentExecution, Threaded};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};
/// Awaited listener after state reduction.
pub type AgentListener<C = crate::NoCustomMessages, E = Threaded> =
    <E as maestro_models::Execution>::Shared<<E as AgentExecution>::Listener<C>>;
/// Live callbacks indexed by monotonically increasing registration order.
pub(super) struct Listeners<C: CustomAgentMessages, E: AgentExecution> {
    /// Next registration identity.
    next: u64,
    /// Current registrations.
    entries: BTreeMap<u64, AgentListener<C, E>>,
}
impl<C: CustomAgentMessages, E: AgentExecution> Default for Listeners<C, E> {
    fn default() -> Self {
        Self {
            next: 0,
            entries: BTreeMap::new(),
        }
    }
}
/// Explicit removal handle; dropping it leaves the listener registered.
pub struct Subscription<
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: AgentExecution = Threaded,
> {
    /// Non-owning registry owner.
    agent: E::Weak<Agent<C, E>>,
    /// Callback identity, including subsequent re-registration.
    listener: E::Weak<E::Listener<C>>,
}
impl<C: CustomAgentMessages, E: AgentExecution> Agent<C, E> {
    /// Register one callback identity in source order.
    pub(crate) fn register(&self, listener: AgentListener<C, E>) {
        let mut listeners = lock(&self.listeners);
        if !listeners
            .entries
            .values()
            .any(|old| std::ptr::addr_eq(&raw const **old, &raw const *listener))
        {
            let id = listeners.next;
            listeners.next += 1;
            listeners.entries.insert(id, listener);
        }
    }
    /// Remove the current registration with the retained weak identity.
    pub(crate) fn remove_listener(&self, pointer: *const E::Listener<C>) {
        let removed = {
            let mut listeners = lock(&self.listeners);
            let key = listeners.entries.iter().find_map(|(key, value)| {
                std::ptr::addr_eq(&raw const **value, pointer).then_some(*key)
            });
            key.and_then(|key| listeners.entries.remove(&key))
        };
        drop(removed);
    }
}
/// Concrete weak references share registration and removal.
macro_rules! entry {
    ($mode:ty,$ptr:ident) => {
        impl<C: CustomAgentMessages> Agent<C, $mode> {
            /// Register a live listener without making the handle own the agent.
            pub fn subscribe(
                self: &$ptr<Self>,
                listener: AgentListener<C, $mode>,
            ) -> Subscription<C, $mode> {
                let result = Subscription {
                    agent: $ptr::downgrade(self),
                    listener: $ptr::downgrade(&listener),
                };
                self.register(listener);
                result
            }
        }
        impl<C: CustomAgentMessages> Subscription<C, $mode> {
            /// Remove the current registration; repeated removal does nothing.
            pub fn unsubscribe(&self) {
                if let Some(agent) = self.agent.upgrade() {
                    agent.remove_listener(self.listener.as_ptr());
                }
            }
        }
    };
}
use std::rc::Rc;
entry!(crate::Local, Rc);
#[cfg(not(target_arch = "wasm32"))]
entry!(Threaded, Arc);
#[cfg(target_arch = "wasm32")]
entry!(Threaded, Rc);
impl<C: CustomAgentMessages + 'static, E: AgentExecution> Agent<C, E> {
    /// Reduce and invoke the first listener now; await live successors without guards.
    pub(crate) fn emit(
        agent: E::Shared<Self>,
        event: AgentEvent<C>,
        signal: Cancellation,
    ) -> impl std::future::Future<Output = Result<(), DiagnosticErrorInfo>> + use<C, E> {
        agent.reduce(&event);
        let event = Arc::new(RwLock::new(event));
        let mut next = 0;

        let mut invoke_next = move || {
            let listener = lock(&agent.listeners)
                .entries
                .range(next..)
                .next()
                .map(|(id, listener)| (*id, listener.clone()));
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
