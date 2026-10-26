//! Canonical reverse read operations preserve bytes and structured failures.
use crate::guest_calls::{
    Callback,
    read::{Invocation, Operation, Output},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
use wasmtime_wasi::WasiView;

/// Exact asynchronous read observation signature.
type Read = TypedFunc<
    (Resource<Callback>, Operation, String),
    (Result<Output, crate::schema_wire::Error>,),
>;
/// Real read operation export on the original instance.
pub(super) struct Exports(Read);
impl Exports {
    /// Resolves the callback on the same guest world as ordinary tool invocation.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(super::typed(store, instance, guest, "invoke-read")?))
    }
    /// Borrows the retained owner until its actual asynchronous callback settles.
    pub(super) async fn invoke<T: WasiView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        let handler = accessor.with(|mut access| access.get().ctx().table.push(call.handler))?;
        let result = self
            .0
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(handler.rep()),
                    call.operation,
                    call.path,
                ),
            )
            .await;
        accessor.with(|mut access| access.get().ctx().table.delete(handler))?;
        let _ = call.reply.send(Ok(result?.0.map_err(Into::into)));
        Ok(())
    }
}
