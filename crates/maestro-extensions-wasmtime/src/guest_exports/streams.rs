//! Canonical authored-stream callbacks use the existing lifecycle reply owner.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    stream_calls::{Invocation, Operation, WireSelection},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// One synchronous classifier or extractor over an original retained event.
type Invoke = TypedFunc<
    (Resource<Callback>, Resource<Callback>, Operation),
    (Result<WireSelection, String>,),
>;
/// The original instance's stream callback export.
pub(crate) struct Exports(Invoke);
impl Exports {
    /// Resolve the actual compiled guest export.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(super::typed(store, instance, guest, "invoke-stream")?))
    }
    /// Invoke with independently retained operands, then settle outside Store access.
    pub(crate) async fn invoke<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        let (handler, value) = a.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((table.push(call.handler)?, table.push(call.value)?))
        })?;
        let result = self
            .0
            .call_concurrent(
                a,
                (
                    Resource::new_borrow(handler.rep()),
                    Resource::new_borrow(value.rep()),
                    call.operation,
                ),
            )
            .await;
        a.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(handler)?;
            table.delete(value)?;
            Ok::<_, wasmtime::Error>(())
        })?;
        let result = match result?.0 {
            Ok(value) => Ok(a.with(|mut access| value.retained(access.get()))?),
            Err(error) => Err(error),
        };
        let _ = call.reply.send(result);
        Ok(())
    }
}
