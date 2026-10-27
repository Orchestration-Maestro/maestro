//! Actual registered-provider and returned-stream exports on the original component.
use crate::{
    callback_imports::{CallbackView, transferred},
    guest_calls::Callback,
    provider_calls::{Assistant, Invocation, Operation, WireInput},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Original provider setup with typed native controls and returned resource.
type Invoke =
    TypedFunc<(Resource<Callback>, String, WireInput), (Result<Resource<Callback>, String>,)>;
/// Original returned-stream method, without a forwarding buffer.
type Stream = TypedFunc<
    (
        Resource<Callback>,
        Operation,
        Option<String>,
        Option<Resource<crate::local_resources::SignalEndpoint>>,
    ),
    (Result<Option<String>, String>,),
>;
/// Canonical exports from the same instance as the registration.
pub(crate) struct Exports {
    /// Original setup callback.
    invoke: Invoke,
    /// Original authored-stream operations.
    stream: Stream,
}
impl Exports {
    /// Resolve both exports on the actual compiled guest.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self {
            invoke: super::typed(store, instance, guest, "invoke-provider")?,
            stream: super::typed(store, instance, guest, "invoke-assistant")?,
        })
    }
    /// Transfer the actual returned resource before settling native synchronous setup.
    pub(crate) async fn invoke<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        let (handler, input) = a.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((table.push(call.handler)?, call.input.lower(table)?))
        })?;
        let result = self
            .invoke
            .call_concurrent(a, (Resource::new_borrow(handler.rep()), call.data, input))
            .await;
        a.with(|mut access| access.get().ctx().table.delete(handler))?;
        let result = match result?.0 {
            Ok(stream) => Ok(a.with(|mut access| transferred(access.get(), stream))?),
            Err(error) => Err(error),
        };
        let _ = call.reply.send(result);
        Ok(())
    }
    /// Await one operation on the original retained resource, under ordinary lifecycle completion.
    pub(crate) async fn assistant<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        call: Assistant<Callback>,
    ) -> wasmtime::Result<()> {
        let handler = a.with(|mut access| access.get().ctx().table.push(call.handler))?;
        let signal = a.with(|mut access| {
            call.signal
                .map(|signal| access.get().ctx().table.push(signal))
                .transpose()
        })?;
        let result = self
            .stream
            .call_concurrent(
                a,
                (
                    Resource::new_borrow(handler.rep()),
                    call.operation,
                    call.data,
                    signal,
                ),
            )
            .await;
        a.with(|mut access| access.get().ctx().table.delete(handler))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
