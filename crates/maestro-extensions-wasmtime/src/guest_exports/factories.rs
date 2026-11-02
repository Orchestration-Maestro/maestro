//! The canonical component factory export, entered with its lent terminal and theme.
use super::arguments;
use crate::guest_calls::{
    Callback,
    factories::{Built, FactoryCall, RetainedComponent},
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

/// Exact canonical tool rendering signature.
type Render = TypedFunc<
    (
        Resource<Callback>,
        crate::guest_calls::tool_renders::Render,
        Resource<ThemeEndpoint>,
        Resource<crate::local_resources::effects::InvalidatorEndpoint>,
        Option<Resource<crate::native_renders::RowEndpoint>>,
        Resource<EmitterEndpoint>,
    ),
    (Result<crate::guest_calls::tool_renders::Rendered, String>,),
>;

/// The component factory and tool rendering exports of the canonical guest instance.
pub(super) struct Exports(Factory, Render);

impl Exports {
    /// Resolves the real guest export.
    pub(super) fn new<T: 'static>(
        store: &mut Store<T>,
        instance: &Instance,
        guest: &ComponentExportIndex,
    ) -> wasmtime::Result<Self> {
        Ok(Self(
            super::typed(store, instance, guest, "invoke-component-factory")?,
            super::typed(store, instance, guest, "invoke-tool-render")?,
        ))
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

impl Exports {
    /// Transfers the theme and redraw capability, borrows the callback and frame, and retains
    /// the returned component and new row state.
    pub(super) async fn render<T: crate::callback_imports::CallbackView + 'static>(
        &self,
        accessor: &Accessor<T>,
        call: crate::guest_calls::tool_renders::ToolRenderCall<Callback>,
    ) -> wasmtime::Result<()> {
        let (callback, theme) = arguments(accessor, &call.handler, call.theme)?;
        let (invalidator, native_row, emitter) = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            Ok::<_, wasmtime::Error>((
                table.push(call.invalidator)?,
                call.native_row.map(|row| table.push(row)).transpose()?,
                table.push(call.emitter)?,
            ))
        })?;
        let operands = (
            Resource::new_borrow(callback.rep()),
            call.render,
            theme,
            invalidator,
            native_row,
            Resource::new_borrow(emitter.rep()),
        );
        let result = self.1.call_concurrent(accessor, operands).await;
        let result = accessor.with(|mut access| {
            let table = access.get().ctx().table;
            table.delete(callback)?;
            table.delete(emitter)?;
            match result?.0 {
                Ok(rendered) => Ok(Ok(rendered_owners(access.get(), rendered)?)),
                Err(error) => Ok::<_, wasmtime::Error>(Err(error)),
            }
        })?;
        let _ = call.reply.send(result);
        Ok(())
    }
}

/// Retains a rendered component and a new row's state as native owners.
pub(crate) fn rendered_owners<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    rendered: crate::guest_calls::tool_renders::Rendered,
) -> wasmtime::Result<(
    Result<crate::guest_calls::Handle, String>,
    Option<crate::guest_calls::Handle>,
)> {
    let component = match rendered.component {
        Ok(component) => Ok(crate::callback_imports::transferred(store, component)?),
        Err(error) => Err(error),
    };
    let row = rendered
        .row
        .map(|row| crate::callback_imports::transferred(store, row))
        .transpose()?;
    Ok((component, row))
}

/// Retains a built component and its callbacks beside the supplied width.
pub(crate) fn retained<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    built: Built,
) -> wasmtime::Result<RetainedComponent> {
    let component = crate::callback_imports::transferred(store, built.component)?;
    let dispose = built
        .dispose
        .map(|dispose| crate::callback_imports::transferred(store, dispose))
        .transpose()?;
    let set_expanded = built
        .set_expanded
        .map(|handler| crate::callback_imports::transferred(store, handler))
        .transpose()?;
    Ok(RetainedComponent {
        component,
        dispose,
        set_expanded,
        width: built.width,
    })
}
