//! Synchronous authored-stream callbacks on the existing Store lifecycle and import route.
use crate::{
    guest_calls::{Call, Callback, Handle},
    lifecycle::replies::Reply,
    local_resources::Registry,
};
use wasmtime::component::{ComponentType, Lift, Lower, Resource};

/// Native buffer callback requested from the author.
#[derive(Clone, Copy, ComponentType, Lift, Lower)]
#[component(enum)]
#[repr(u8)]
pub(crate) enum Operation {
    /// Recognize a terminal event synchronously.
    #[component(name = "classify")]
    Classify,
    /// Extract the original retained result synchronously.
    #[component(name = "extract")]
    Extract,
}
/// Result retained by the original native invocation.
pub(crate) enum Selection {
    /// Terminal recognition.
    Complete(bool),
    /// Extracted caller-local value.
    Extracted(Handle),
}
/// Canonical result of the author's synchronous callback.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(crate) enum WireSelection {
    /// Terminal recognition.
    #[component(name = "complete")]
    Complete(bool),
    /// Independently owned extracted value.
    #[component(name = "extracted")]
    Extracted(Resource<Handle>),
}
impl WireSelection {
    /// Transfer the result's native ownership before exposing it to the waiting caller.
    pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
        self,
        store: &mut T,
    ) -> wasmtime::Result<Selection> {
        Ok(match self {
            Self::Complete(value) => Selection::Complete(value),
            Self::Extracted(value) => Selection::Extracted(store.ctx().table.delete(value)?),
        })
    }
}
/// Owned operands with their original synchronous settlement.
pub(crate) struct Invocation<H = Handle> {
    /// Original callback, independent of the currently running handler.
    pub(crate) handler: H,
    /// Original event, without JSON or Clone bounds on its guest type.
    pub(crate) value: H,
    /// Native phase selecting the callback.
    pub(crate) operation: Operation,
    /// Original lifecycle reply.
    pub(crate) reply: Reply<Selection>,
}
impl Handle {
    /// Invoke through the current synchronous route while servicing local reentry.
    pub(crate) fn stream_call(
        &self,
        registry: &Registry,
        value: Handle,
        operation: Operation,
    ) -> Result<Selection, String> {
        self.endpoint().synchronous(registry, |reply| {
            Ok(Call::Stream(Invocation {
                handler: self.clone(),
                value,
                operation,
                reply,
            }))
        })
    }
}
/// Canonical operands of a nested same-instance callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Local {
    /// Existing classifier or extractor identity.
    pub(crate) target: u32,
    /// Existing event identity in the same guest's callback table.
    pub(crate) value: u32,
    /// Synchronous native phase.
    pub(crate) operation: Operation,
}
impl Local {
    /// Borrow only the stable identities; the invocation completion retains both owners.
    pub(crate) fn new(call: &Invocation<Callback>) -> Self {
        Self {
            target: call.handler.id,
            value: call.value.id,
            operation: call.operation,
        }
    }
}
