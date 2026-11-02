//! The canonical widget callback export, entered with its borrowed invocation frame.
use super::arguments;
use crate::guest_calls::{
    Callback,
    widget_calls::{Answer, WidgetCall, WireEvent, answered},
};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact canonical widget callback signature.
type Widget = TypedFunc<
    (Resource<Callback>, WireEvent, Resource<EmitterEndpoint>),
    (Result<Answer, String>,),
>;

/// The widget callback export of the canonical guest instance.
pub(super) struct Exports(Widget);

impl Exports {
    /// Resolves the real guest export.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        super::typed(store, instance, guest, "invoke-widget").map(Self)
    }

    /// Runs one callback, deleting its temporary borrows before replying.
    pub(super) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: WidgetCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, emitter) = arguments(accessor, &call.handler, call.emitter)?;
        let event = accessor.with(|mut access| call.event.lower(access.get().ctx().table))?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            event,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.0.call_concurrent(accessor, operands).await;
        let answer = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(answer) => Ok::<_, wasmtime::Error>(Ok(answered(access.get(), answer)?)),
                Err(error) => Ok(Err(error)),
            }
        })?;
        let _ = call.reply.send(answer);
        Ok(())
    }
}
