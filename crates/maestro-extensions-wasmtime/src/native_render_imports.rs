//! Rendering imports preserve original native row and component ownership.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::local_resources::effects::ThemeEndpoint;
use crate::{
    bindings::maestro::extension::host::HostNativeComponent,
    builtin_reads::Endpoint,
    callback_imports::CallbackView,
    context_imports::add_resource,
    import_route::{Route, push},
    lifecycle::State,
    native_renders::{ComponentEndpoint, Previous, Render, RowEndpoint, Slot},
    read_imports::{endpoint, forwards, release},
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
    Ok(())
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

impl HostNativeComponent for State {
    fn render(
        &mut self,
        this: Resource<ComponentEndpoint>,
        width: u32,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.render(width);
        push(self, route)
    }

    fn handle_input(
        &mut self,
        this: Resource<ComponentEndpoint>,
        data: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.handle_input(data);
        push(self, route)
    }

    fn invalidate(
        &mut self,
        this: Resource<ComponentEndpoint>,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.invalidate();
        push(self, route)
    }

    fn wants_key_release(
        &mut self,
        this: Resource<ComponentEndpoint>,
    ) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.wants_key_release();
        push(self, route)
    }

    fn drop(&mut self, this: Resource<ComponentEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every native component method to its trait implementation.
pub(crate) fn add_component(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, ComponentEndpoint>(host, "native-component")?;
    forwards! { host, "native-component", HostNativeComponent, ComponentEndpoint,
        "render" => render(width: u32);
        "handle-input" => handle_input(data: String);
        "invalidate" => invalidate();
        "wants-key-release" => wants_key_release();
    }
    Ok(())
}
