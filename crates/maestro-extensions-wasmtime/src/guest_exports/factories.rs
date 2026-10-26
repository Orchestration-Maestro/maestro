//! The canonical component factory export, entered with its lent terminal and theme.
use super::arguments;
use crate::guest_calls::{
    Callback,
    factories::{Built, FactoryCall},
};
use crate::local_resources::effects::{EmitterEndpoint, ThemeEndpoint, TuiEndpoint};
use wasmtime::{
    Store,
    component::{Accessor, ComponentExportIndex, Instance, Resource, TypedFunc},
};

/// Exact canonical factory signature.
type Factory = TypedFunc<
    (
        Resource<Callback>,
        Resource<TuiEndpoint>,
        Resource<ThemeEndpoint>,
        Option<Resource<crate::footer_data::Endpoint>>,
        Resource<EmitterEndpoint>,
    ),
    (Result<Built, String>,),
>;

/// The component factory export of the canonical guest instance.
pub(super) struct Exports(Factory);

impl Exports {
    /// Resolves the real guest export.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        super::typed(store, instance, guest, "invoke-component-factory").map(Self)
    }

    /// Transfers the terminal and theme, borrows the callback and frame, and retains the
    /// returned identities.
    pub(super) async fn invoke<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: FactoryCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, tui) = arguments(accessor, &call.handler, call.tui)?;
        let (theme, footer, emitter) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            let footer = call.footer.map(|footer| table.push(footer)).transpose()?;
            Ok::<_, wasmtime::Error>((table.push(call.theme)?, footer, table.push(call.emitter)?))
        })?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            tui,
            theme,
            footer,
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
}

/// Retains a built component and its optional disposal as native owners.
pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    built: Built,
) -> wasmtime::Result<(
    crate::guest_calls::Handle,
    Option<crate::guest_calls::Handle>,
)> {
    let component = crate::callback_imports::transferred(store, built.component)?;
    let dispose = built
        .dispose
        .map(|dispose| crate::callback_imports::transferred(store, dispose))
        .transpose()?;
    Ok((component, dispose))
}
