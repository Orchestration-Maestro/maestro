//! Canonical schema exports, with no duplicated checker or callback storage.
use super::typed;
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    local_resources::effects::EmitterEndpoint,
    schema_reads::Endpoint,
    schema_wire::{Error, Operation, Shape},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};
/// The canonical schema invocation signature.
type Invoke = TypedFunc<
    (
        Resource<Callback>,
        Operation,
        Resource<Endpoint>,
        Resource<EmitterEndpoint>,
    ),
    (Result<String, Error>,),
>;
/// The canonical data-only shape query.
type Describe = TypedFunc<(Resource<Callback>,), (Result<Shape, Error>,)>;
/// The canonical callback alias operation.
type Retain = TypedFunc<(Resource<Callback>, Resource<Callback>), (Result<(), Error>,)>;
/// The schema methods of the original guest instance.
pub(crate) struct Exports {
    /// The original schema invocation export.
    invoke: Invoke,
    /// The guest method shape entry.
    pub(crate) describe: Describe,
    /// The original schema retention export.
    retain: Retain,
}
impl Exports {
    /// Resolve canonical schema exports from this instance.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            invoke: typed(store, instance, guest, "invoke-schema")?,
            describe: typed(store, instance, guest, "describe-schema")?,
            retain: typed(store, instance, guest, "retain-schema")?,
        })
    }
    /// The guest method shape entry.
    pub(crate) async fn describe<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        handler: Resource<Callback>,
    ) -> wasmtime::Result<Result<Shape, Error>> {
        Ok(self.describe.call_concurrent(a, (handler,)).await?.0)
    }
    /// The guest alias-retention entry.
    pub(crate) async fn retain<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        handler: Resource<Callback>,
        retained: Resource<Callback>,
    ) -> wasmtime::Result<Result<(), Error>> {
        Ok(self.retain.call_concurrent(a, (handler, retained)).await?.0)
    }
    /// The guest method invocation entry.
    pub(crate) async fn invoke<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        call: crate::guest_calls::schema::Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        #[cfg(test)]
        eprintln!("store schema start {}", call.handler.id);
        let (callback, node, emitter) = a.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                table.push(call.handler)?,
                table.push(call.node)?,
                table.push(call.emitter)?,
            ))
        })?;
        let result = self
            .invoke
            .call_concurrent(
                a,
                (
                    Resource::new_borrow(callback.rep()),
                    call.operation,
                    Resource::new_borrow(node.rep()),
                    Resource::new_borrow(emitter.rep()),
                ),
            )
            .await;
        a.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(node)?;
            table.delete(emitter)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        #[cfg(test)]
        eprintln!("store schema returned");
        let _ = call.reply.send(Ok(result?.0));
        Ok(())
    }
}
