//! Real awaited model-hook exports on the original component instance.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    local_resources::SignalEndpoint,
    model_calls::{Invocation, Phase, Selection, WireSelection},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Awaited selector or observer with independently owned cancellation.
type Invoke = TypedFunc<
    (
        Resource<Callback>,
        Phase,
        String,
        String,
        Option<Resource<SignalEndpoint>>,
    ),
    (Result<WireSelection, String>,),
>;
/// Required authored model-hook export.
pub(crate) struct Exports(Invoke);
impl Exports {
    /// Resolve the actual guest method on this instance.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(super::typed(
            store,
            instance,
            guest,
            "invoke-model-hook",
        )?))
    }
    /// Call the original closure and transfer all returned resource ownership before settlement.
    pub(crate) async fn invoke<T: CallbackView + 'static>(
        &self,
        a: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        let (handler, signal) = a.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                table.push(call.handler)?,
                call.signal.map(|signal| table.push(signal)).transpose()?,
            ))
        })?;
        let result = self
            .0
            .call_concurrent(
                a,
                (
                    Resource::new_borrow(handler.rep()),
                    call.phase,
                    call.data,
                    call.model,
                    signal,
                ),
            )
            .await;
        a.with(|mut access| access.get().ctx().table.delete(handler))?;
        let result = match result?.0 {
            Ok(selected) => Ok(a.with(|mut access| {
                let state = access.get();
                let signal = selected
                    .signal
                    .map(|signal| state.ctx().table.delete(signal))
                    .transpose()?;
                let owner = selected
                    .owner
                    .map(|owner| crate::callback_imports::transferred(state, owner))
                    .transpose()?;
                Ok::<_, wasmtime::Error>(Selection {
                    data: selected.data,
                    signal,
                    owner,
                })
            })?),
            Err(error) => Err(error),
        };
        let _ = call.reply.send(result);
        Ok(())
    }
}
