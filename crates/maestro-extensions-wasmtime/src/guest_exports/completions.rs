//! The canonical completion provider and provider factory exports.
use super::arguments;
use crate::guest_calls::{
    Callback,
    providers::{Answer, FactoryCall, ProviderCall, ProviderEndpoint, Request},
};
use crate::local_resources::{SignalEndpoint, effects::EmitterEndpoint};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact canonical provider request signature.
type Provider = TypedFunc<
    (
        Resource<Callback>,
        Request,
        Option<Resource<SignalEndpoint>>,
    ),
    (Result<Answer, String>,),
>;

/// Exact canonical provider factory signature.
type Factory = TypedFunc<
    (
        Resource<Callback>,
        Resource<ProviderEndpoint>,
        Resource<EmitterEndpoint>,
    ),
    (Result<Resource<Callback>, String>,),
>;

/// The provider and provider factory exports of the canonical guest instance.
pub(super) struct Exports(Provider, Factory);

impl Exports {
    /// Resolves the real guest exports.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(
            super::typed(store, instance, guest, "invoke-autocomplete")?,
            super::typed(store, instance, guest, "invoke-autocomplete-factory")?,
        ))
    }

    /// Runs one request, transferring a suggestion request's signal.
    pub(super) async fn request<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: ProviderCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, signal) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            let callback = table.push(Callback {
                id: call.handler.id,
                schema: None,
            })?;
            let signal = call.signal.map(|signal| table.push(signal)).transpose()?;
            Ok::<_, wasmtime::Error>((callback, signal))
        })?;
        let operands = (Resource::new_borrow(callback.rep()), call.request, signal);
        let result = self.0.call_concurrent(accessor, operands).await;
        accessor.with(|mut access| access.get().ctx().table.delete(callback))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }

    /// Lends the current provider, borrows the callback and frame, and keeps the returned
    /// provider as a native owner.
    pub(super) async fn factory<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: FactoryCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, current) = arguments(accessor, &call.handler, call.current)?;
        let emitter = accessor.with(|mut access| access.get().ctx().table.push(call.emitter))?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            current,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.1.call_concurrent(accessor, operands).await;
        let provider = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(provider) => Ok(Ok(crate::callback_imports::transferred(
                    access.get(),
                    provider,
                )?)),
                Err(error) => Ok::<_, wasmtime::Error>(Err(error)),
            }
        })?;
        let _ = call.reply.send(provider);
        Ok(())
    }
}
