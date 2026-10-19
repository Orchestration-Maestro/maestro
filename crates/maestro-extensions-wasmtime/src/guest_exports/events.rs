//! Canonical event invocation transfers the original context and optional signal.
use super::arguments;
use crate::{
    context_imports::Context,
    guest_calls::{Callback, EventCall, Outcome},
    local_resources::SignalEndpoint,
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lower, Resource, TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

/// Exact canonical owned capability record.
#[derive(ComponentType, Lower)]
#[component(record)]
struct Capabilities {
    /// Original invocation context, not loading defaults.
    ctx: Resource<Context>,
    /// Independently owned original signal.
    signal: Option<Resource<SignalEndpoint>>,
}
/// Exact async guest event signature.
type Invoke = TypedFunc<(Resource<Callback>, String, Capabilities), (Result<Outcome, String>,)>;
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
        let index = instance
            .get_export_index(&mut *store, Some(guest), "invoke-event")
            .ok_or_else(|| wasmtime::format_err!("missing extension invoke-event"))?;
        Ok(Self {
            invoke: instance.get_typed_func(store, index)?,
        })
    }
    /// Retains callback ownership through completion and removes its temporary borrow.
    pub(super) async fn invoke<T: WasiView + 'static>(
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
        accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
