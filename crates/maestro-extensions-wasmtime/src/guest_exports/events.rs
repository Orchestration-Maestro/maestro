//! Canonical event invocation transfers the original context and optional signal.
use super::arguments;
use crate::{
    context_imports::Context,
    guest_calls::{Callback, EventCall, events::Returned},
    local_resources::SignalEndpoint,
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lower, Resource, TypedFunc,
    },
};

/// Exact canonical owned capability record.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Capabilities {
    /// Original invocation context, not loading defaults.
    pub(crate) ctx: Resource<Context>,
    /// Independently owned original signal.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
}
/// Exact async guest event signature.
type Invoke = TypedFunc<(Resource<Callback>, String, Capabilities), (Result<Returned, String>,)>;
/// The actual canonical event export.
pub(super) struct Exports {
    /// The same instance's authored event handler.
    invoke: Invoke,
}
impl Exports {
    /// Resolves the canonical guest export on the original instance.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            invoke: super::typed(store, instance, guest, "invoke-event")?,
        })
    }
    /// Retains callback ownership through completion and removes its temporary borrow.
    pub(super) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: EventCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, ctx) = arguments(accessor, &call.handler, Context(call.context))?;
        let signal = accessor.with(|mut access| {
            call.signal
                .map(|signal| access.get().ctx().table.push(signal))
                .transpose()
        })?;
        let result = self
            .invoke
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(callback.rep()),
                    call.event,
                    Capabilities { ctx, signal },
                ),
            )
            .await;
        let outcome = accessor.with(|mut access| {
            access.get().ctx().table.delete(callback)?;
            Ok::<_, wasmtime::Error>(match result?.0 {
                Ok(returned) => Ok(returned.retained(access.get())?),
                Err(error) => Err(error),
            })
        })?;
        let _ = call.reply.send(outcome);
        Ok(())
    }
}
