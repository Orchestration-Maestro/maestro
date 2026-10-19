//! Canonical returned shell execution uses the real guest resource and original capabilities.
use crate::{
    guest_calls::{BashCall, BashRequest},
    local_resources::{SignalEndpoint, bash_output},
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lower, Resource, ResourceAny,
        TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

/// Independently owned original resources for this one execution.
#[derive(ComponentType, Lower)]
#[component(record)]
struct Capabilities {
    /// Original byte-stream frame, not a stored borrowed sink.
    output: Resource<bash_output::Endpoint>,
    /// Independently optional original signal.
    signal: Option<Resource<SignalEndpoint>>,
}

/// Actual canonical async method on a guest-owned shell operation.
type Execute = TypedFunc<(ResourceAny, BashRequest, Capabilities), (Result<Option<f64>, String>,)>;

/// Required canonical guest resource method.
pub(super) struct Exports {
    /// Real method resolved from the original instance.
    execute: Execute,
}

impl Exports {
    /// Resolves the actual guest resource method, with no substitute implementation.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        let index = instance
            .get_export_index(&mut *store, Some(guest), "[method]bash-operations.exec")
            .ok_or_else(|| wasmtime::format_err!("missing extension bash-operations.exec"))?;
        Ok(Self {
            execute: instance.get_typed_func(&mut *store, index)?,
        })
    }

    /// Borrows the same guest owner during overlapping calls; final release stays queued.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: BashCall,
    ) -> wasmtime::Result<()> {
        let resources = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>(Capabilities {
                output: table.push(call.output)?,
                signal: call.signal.map(|signal| table.push(signal)).transpose()?,
            })
        })?;
        let result = self
            .execute
            .call_concurrent(accessor, (call.owner.resource, call.request, resources))
            .await;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
