//! Canonical returned shell execution uses its authored identity and original capabilities.
use crate::{
    guest_calls::{BashCall, BashRequest, Callback},
    local_resources::{SignalEndpoint, bash_output},
};
use wasmtime::{
    Store,
    component::{
        Accessor, ComponentExportIndex, ComponentType, Instance, Lower, Resource, TypedFunc,
    },
};
use wasmtime_wasi::WasiView;

/// Independently owned original resources for this one execution.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct Capabilities {
    /// Original byte-stream frame, not a stored borrowed sink.
    pub(crate) output: Resource<bash_output::Endpoint>,
    /// Independently optional original signal.
    pub(crate) signal: Option<Resource<SignalEndpoint>>,
}

/// Actual canonical async method on a guest-owned shell operation.
type Execute =
    TypedFunc<(Resource<Callback>, BashRequest, Capabilities), (Result<Option<f64>, String>,)>;

/// Required canonical shell-execution export.
pub(super) struct Exports {
    /// Real method resolved from the original instance.
    execute: Execute,
}

impl Exports {
    /// Resolves the authored shell-execution export from the original instance.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            execute: super::typed(store, instance, guest, "bash-exec")?,
        })
    }

    /// Borrows the same guest owner during overlapping calls; final release stays queued.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: BashCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (owner, resources) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                table.push(call.owner)?,
                Capabilities {
                    output: table.push(call.output)?,
                    signal: call.signal.map(|signal| table.push(signal)).transpose()?,
                },
            ))
        })?;
        let result = self
            .execute
            .call_concurrent(
                accessor,
                (Resource::new_borrow(owner.rep()), call.request, resources),
            )
            .await;
        accessor.with(|mut access| access.get().ctx().table.delete(owner))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
