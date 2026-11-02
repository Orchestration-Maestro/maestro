//! The canonical terminal input export, entered with its borrowed invocation frame.
use super::arguments;
use crate::guest_calls::{
    Callback,
    terminal::{Decision, Input},
};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
use wasmtime_wasi::WasiView;

/// Exact canonical terminal input signature.
type Listener = TypedFunc<
    (Resource<Callback>, String, Resource<EmitterEndpoint>),
    (Result<Option<Decision>, String>,),
>;

/// The terminal input export of the canonical guest instance.
pub(super) struct Exports(Listener);

impl Exports {
    /// Resolves the real guest export.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        super::typed(store, instance, guest, "invoke-terminal-input").map(Self)
    }

    /// Runs one listener on one chunk, deleting its temporary borrows before replying.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Input<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, emitter) = arguments(accessor, &call.handler, call.emitter)?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            call.data,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.0.call_concurrent(accessor, operands).await;
        accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
