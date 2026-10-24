//! Identity-deduplicated live subscription traversal.
use super::{Agent, CustomAgentMessages, lock};
use crate::AgentEvent;
use crate::types::{AgentExecution, Threaded};
use maestro_models::{Cancellation, DiagnosticErrorInfo};
use std::sync::{Arc, RwLock};
/// Awaited listener after state reduction.
pub type AgentListener<C = crate::NoCustomMessages, E = Threaded> =
    <E as maestro_models::Execution>::Shared<<E as AgentExecution>::Listener<C>>;
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
        lock(&self.listeners).add(listener);
    }
    /// Remove the current registration with the retained weak identity.
    pub(crate) fn remove_listener(&self, pointer: *const E::Listener<C>) {
        let removed = lock(&self.listeners).remove(pointer);
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
        let mut cursor = lock(&agent.listeners).cursor();

        let mut invoke_next = move || {
            let listener = lock(&agent.listeners).next_from(&mut cursor).cloned();
            listener.map(|listener| listener(Arc::clone(&event), signal.clone()))
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
