//! Canonical custom factory prefixes and their independent asynchronous continuations.
use crate::guest_calls::{
    Callback,
    custom::{Built, Invocation, Outcome, Prefix as PrefixCall, WireInput},
    factories::RetainedComponent,
};
use crate::{lifecycle::replies::Reply, local_resources::effects::EmitterEndpoint};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact prefix signature, including the borrowed callback frame.
type Prefix =
    TypedFunc<(Resource<Callback>, WireInput, Resource<EmitterEndpoint>), (Result<Built, String>,)>;
/// Exact owned continuation signature, without an emission frame.
type Continuation =
    TypedFunc<(Resource<Callback>,), (Result<crate::guest_calls::factories::Built, String>,)>;
/// Required exports from the original canonical instance.
pub(super) struct Exports(Prefix, Continuation);
impl Exports {
    /// Resolves the canonical prefix and continuation exports.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(
            super::typed(store, instance, guest, "invoke-custom-factory")?,
            super::typed(store, instance, guest, "invoke-custom-future")?,
        ))
    }
    /// Dispatches prefixes and continuations through the same retained Store.
    pub(super) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        match call {
            Invocation::Prefix(call) => self.prefix(accessor, call).await,
            Invocation::Future(handler, reply) => self.future(accessor, handler, reply).await,
        }
    }
    /// Runs only the prefix, retiring its borrowed callback frame before settlement.
    async fn prefix<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: PrefixCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, emitter) = super::arguments(accessor, &call.handler, call.emitter)?;
        let input = accessor.with(|mut access| call.input.transfer(access.get().ctx().table))?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            input,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.0.call_concurrent(accessor, operands).await;
        let result = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(built) => Ok(Ok(retained(access.get(), built)?)),
                Err(error) => Ok::<_, wasmtime::Error>(Err(error)),
            }
        })?;
        let _ = call.reply.send(result);
        Ok(())
    }
    /// Awaits the owned continuation without extending its prefix's emitter lifetime.
    async fn future<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        handler: Callback,
        reply: Reply<RetainedComponent>,
    ) -> wasmtime::Result<()> {
        let callback = accessor.with(|mut access| {
            access.get().ctx().table.push(Callback {
                id: handler.id,
                schema: None,
            })
        })?;
        let result = self
            .1
            .call_concurrent(accessor, (Resource::new_borrow(callback.rep()),))
            .await;
        let result = accessor.with(|mut access| {
            access.get().ctx().table.delete(callback)?;
            match result?.0 {
                Ok(built) => Ok(Ok(super::retained(access.get(), built)?)),
                Err(error) => Ok::<_, wasmtime::Error>(Err(error)),
            }
        })?;
        let _ = reply.send(result);
        Ok(())
    }
}
/// Converts returned callback resources to their actual final-consumer owners.
pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    built: Built,
) -> wasmtime::Result<Outcome> {
    match built {
        Built::Ready(built) => super::retained(store, built).map(Outcome::Ready),
        Built::Pending(pending) => {
            crate::callback_imports::transferred(store, pending).map(Outcome::Pending)
        }
    }
}
