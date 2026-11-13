//! Rendering imports preserve original native row and component ownership.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::local_resources::effects::ThemeEndpoint;
use crate::{
    bindings::maestro::extension::host::HostNativeComponent,
    bindings::maestro::extension::host::{
        HostBuiltinTool, PreparedArguments, ToolRenderInfo, ToolRenderResultOptions,
    },
    builtin_reads::Endpoint,
    callback_imports::CallbackView,
    context_imports::add_resource,
    import_route::{Route, push},
    lifecycle::State,
    native_renders::{ComponentEndpoint, Previous, Render, RowEndpoint, Slot},
    read_imports::{endpoint, forwards, release},
};
use wasmtime::component::{LinkerInstance, Resource};

/// Installs opaque row/component resources and their existing native operations.
pub(crate) fn link<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, RowEndpoint>(host, "native-row")?;
    Ok(())
}
impl From<ToolRenderInfo> for crate::guest_calls::tool_renders::Info {
    fn from(info: ToolRenderInfo) -> Self {
        Self {
            tool_call_id: info.tool_call_id,
            args: info.args,
            cwd: info.cwd,
            phase: crate::guest_calls::tool_renders::Phase {
                execution_started: info.phase.execution_started,
                args_complete: info.phase.args_complete,
                is_partial: info.phase.is_partial,
            },
            expanded: info.expanded,
            show_images: info.show_images,
            is_error: info.is_error,
        }
    }
}

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

impl HostBuiltinTool for State {
    fn metadata(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.metadata())
    }

    fn prepare(
        &mut self,
        this: Resource<Endpoint>,
        args: String,
    ) -> wasmtime::Result<Result<PreparedArguments, String>> {
        Ok(endpoint(self, &this)?.prepare(args))
    }

    fn render_call(
        &mut self,
        this: Resource<Endpoint>,
        args: Option<String>,
        info: ToolRenderInfo,
        theme: Resource<ThemeEndpoint>,
        row: Option<Resource<RowEndpoint>>,
        last_native: Option<Resource<ComponentEndpoint>>,
        last_authored: Option<Resource<crate::guest_calls::Callback>>,
    ) -> wasmtime::Result<Resource<Route>> {
        let render = operands(
            self,
            (row, theme, last_native, last_authored),
            info.into(),
            Slot::Call(args),
        )?;
        let tool = endpoint(self, &this)?;
        push(self, tool.render(render))
    }

    fn render_result(
        &mut self,
        this: Resource<Endpoint>,
        value: String,
        options: ToolRenderResultOptions,
        info: ToolRenderInfo,
        theme: Resource<ThemeEndpoint>,
        row: Option<Resource<RowEndpoint>>,
        last_native: Option<Resource<ComponentEndpoint>>,
        last_authored: Option<Resource<crate::guest_calls::Callback>>,
    ) -> wasmtime::Result<Resource<Route>> {
        let options = maestro_extensions::ToolRenderResultOptions {
            expanded: options.expanded,
            is_partial: options.is_partial,
        };
        let render = operands(
            self,
            (row, theme, last_native, last_authored),
            info.into(),
            Slot::Result(value, options),
        )?;
        let tool = endpoint(self, &this)?;
        push(self, tool.render(render))
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
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

/// Registers the tool resource with its synchronous methods forwarded to the trait.
pub(crate) fn add_tool(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "builtin-tool")?;
    forwards! { host, "builtin-tool", HostBuiltinTool, Endpoint,
        "metadata" => metadata();
        "prepare" => prepare(args: String);
        "render-call" => render_call(
            args: Option<String>,
            info: ToolRenderInfo,
            theme: Resource<ThemeEndpoint>,
            row: Option<Resource<RowEndpoint>>,
            last_native: Option<Resource<ComponentEndpoint>>,
            last_authored: Option<Resource<crate::guest_calls::Callback>>
        );
        "render-result" => render_result(
            value: String,
            options: ToolRenderResultOptions,
            info: ToolRenderInfo,
            theme: Resource<ThemeEndpoint>,
            row: Option<Resource<RowEndpoint>>,
            last_native: Option<Resource<ComponentEndpoint>>,
            last_authored: Option<Resource<crate::guest_calls::Callback>>
        );
    }
    Ok(())
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
