//! OAuth interaction exports run on the original component instance.
use crate::{
    guest_calls::Callback,
    oauth_callbacks::{Invocation, Phase},
};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact callback input and optional text response.
type Invoke = TypedFunc<
    (
        Resource<Callback>,
        Phase,
        String,
        Option<Resource<crate::oauth_interaction::Interaction>>,
    ),
    (Result<Option<String>, String>,),
>;
/// Actual authored interaction export.
pub(crate) struct Exports(Invoke);
impl Exports {
    /// Resolves the canonical export of this same instance.
    pub(crate) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(super::typed(
            store,
            instance,
            guest,
            "invoke-oauth-callback",
        )?))
    }
    /// Retains the borrowed callback identity until its original result settles.
    pub(crate) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: Invocation<Callback>,
    ) -> wasmtime::Result<()> {
        let handler = accessor.with(|mut access| access.get().ctx().table.push(call.handler))?;
        let interaction = call
            .interaction
            .map(|interaction| {
                accessor.with(|mut access| access.get().ctx().table.push(interaction))
            })
            .transpose()?;
        let result = self
            .0
            .call_concurrent(
                accessor,
                (
                    Resource::new_borrow(handler.rep()),
                    call.phase,
                    call.data,
                    interaction,
                ),
            )
            .await;
        accessor.with(|mut access| access.get().ctx().table.delete(handler))?;
        let _ = call.reply.send(result?.0);
        Ok(())
    }
}
