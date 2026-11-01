//! Rendering imports preserve original native row and component ownership.
use crate::local_resources::effects::ThemeEndpoint;
use crate::{
    builtin_reads::Endpoint,
    callback_imports::CallbackView,
    native_renders::{ComponentEndpoint, ComponentRequest, Previous, Render, RowEndpoint, Slot},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs opaque row/component resources and their existing native operations.
pub(crate) fn link<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, RowEndpoint>(host, "native-row")?;
    crate::context_imports::add_resource::<T, ComponentEndpoint>(host, "native-component")?;
    host.func_wrap(
        "[method]builtin-tool.render-call",
        |mut store: StoreContextMut<'_, T>,
         (tool, args, info, theme, row, native, authored): CallArgs| {
            let render = operands(
                store.data_mut(),
                (row, theme, native, authored),
                info,
                Slot::Call(args),
            )?;
            let tool = store.data_mut().ctx().table.get(&tool)?.clone();
            Ok((crate::import_route::push(
                store.data_mut(),
                tool.render(render),
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]builtin-tool.render-result",
        |mut store: StoreContextMut<'_, T>,
         (tool, result, options, info, theme, row, native, authored): ResultArgs| {
            let options = maestro_extensions::ToolRenderResultOptions {
                expanded: options.expanded,
                is_partial: options.is_partial,
            };
            let render = operands(
                store.data_mut(),
                (row, theme, native, authored),
                info,
                Slot::Result(result, options),
            )?;
            let tool = store.data_mut().ctx().table.get(&tool)?.clone();
            Ok((crate::import_route::push(
                store.data_mut(),
                tool.render(render),
            )?,))
        },
    )?;
    component(host)
}
/// Exact ordinary call operands, independently of the opaque row context.
type CallArgs = (
    Resource<Endpoint>,
    Option<String>,
    crate::guest_calls::tool_renders::Info,
    Resource<ThemeEndpoint>,
    Option<Resource<RowEndpoint>>,
    Option<Resource<ComponentEndpoint>>,
    Option<Resource<crate::guest_calls::Callback>>,
);
/// Exact result operands with their independent options.
type ResultArgs = (
    Resource<Endpoint>,
    String,
    crate::guest_calls::tool_renders::Options,
    crate::guest_calls::tool_renders::Info,
    Resource<ThemeEndpoint>,
    Option<Resource<RowEndpoint>>,
    Option<Resource<ComponentEndpoint>>,
    Option<Resource<crate::guest_calls::Callback>>,
);
/// Original row, theme and previous component resource operands.
type Resources = (
    Option<Resource<RowEndpoint>>,
    Resource<ThemeEndpoint>,
    Option<Resource<ComponentEndpoint>>,
    Option<Resource<crate::guest_calls::Callback>>,
);
/// Resolves each supplied resource before entering any native rendering code.
fn operands<T: CallbackView>(
    store: &mut T,
    (row, theme, native, authored): Resources,
    info: crate::guest_calls::tool_renders::Info,
    slot: Slot,
) -> wasmtime::Result<Render> {
    let table = store.ctx().table;
    let row = row
        .as_ref()
        .map(|row| table.get(row).cloned())
        .transpose()?;
    let theme = table.get(&theme)?.clone();
    let native = native
        .as_ref()
        .map(|last| table.get(last).cloned())
        .transpose()?;
    let authored = authored
        .as_ref()
        .map(|last| crate::callback_imports::retain_object(store, last))
        .transpose()?;
    let last = match (native, authored) {
        (Some(native), None) => Some(Previous::Native(native)),
        (None, Some(authored)) => Some(Previous::Authored(authored)),
        (None, None) => None,
        (Some(_), Some(_)) => {
            return Err(wasmtime::format_err!(
                "native renderer received two previous components"
            ));
        }
    };
    Ok(Render {
        row,
        theme,
        last,
        info,
        slot,
    })
}
/// Forwards native view methods through the existing nested-call route.
fn component<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]native-component.render",
        |mut store: StoreContextMut<'_, T>,
         (resource, width): (Resource<ComponentEndpoint>, u32)| {
            let route = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .call(ComponentRequest::Render(width));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "[method]native-component.handle-input",
        |mut store: StoreContextMut<'_, T>,
         (resource, data): (Resource<ComponentEndpoint>, String)| {
            let route = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .call(ComponentRequest::Input(data));
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    for (name, request) in [
        (
            "[method]native-component.invalidate",
            ComponentRequest::Invalidate,
        ),
        (
            "[method]native-component.wants-key-release",
            ComponentRequest::Key,
        ),
    ] {
        host.func_wrap(
            name,
            move |mut store: StoreContextMut<'_, T>,
                  (resource,): (Resource<ComponentEndpoint>,)| {
                let route = store
                    .data_mut()
                    .ctx()
                    .table
                    .get(&resource)?
                    .call(request.clone());
                Ok((crate::import_route::push(store.data_mut(), route)?,))
            },
        )?;
    }
    Ok(())
}
