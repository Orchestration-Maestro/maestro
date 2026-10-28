//! Tool render callbacks invoked through the same exports as the component host.
use super::super::super::{
    self as tests,
    renderer_host::{Call, Reply},
};
use super::Host;
use serde_json::{Value, json};

impl Host {
    /// Renders a tool call or result through its registered renderer, not its implementation.
    pub(in super::super) fn render(
        &mut self,
        slot: &str,
        args: &Value,
        result: Option<Value>,
        expanded: bool,
    ) -> Result<String, String> {
        let key = self.observe(|o| o.callback(&format!("render-{slot} todo")))?;
        let info = json!({"toolCallId":"paint","cwd":"/work","args":args,"expanded":expanded});
        let lines = match self {
            Self::Controlled(host, exports) => {
                use crate::bindings::maestro::extension::host as wire;
                let render = wire::ToolRender {
                    slot: if slot == "call" {
                        wire::ToolRenderSlot::Call
                    } else {
                        wire::ToolRenderSlot::Result
                    },
                    row: None,
                    args: Some(args.to_string()),
                    result: result.map(|v| v.to_string()),
                    options: Some(wire::ToolRenderResultOptions {
                        expanded,
                        is_partial: false,
                    }),
                    info: wire::ToolRenderInfo {
                        tool_call_id: "paint".into(),
                        cwd: "/work".into(),
                        args: Some(args.to_string()),
                        phase: wire::ToolRenderPhase {
                            execution_started: false,
                            args_complete: true,
                            is_partial: false,
                        },
                        expanded,
                        show_images: false,
                        is_error: false,
                    },
                };
                let (component, _) = exports.invoke_tool_render(
                    key,
                    &render,
                    (host.theme("dark"), host.invalidator("paint"), None),
                    &tests::bus_host::Emitter { key: 0 },
                )?;
                let Reply::Lines(lines) =
                    tests::renderer_host::invoke(&component?, Call::Render(80), host, 0)?
                else {
                    return Err("expected lines".into());
                };
                lines
            }
            Self::Component(host) => render_component(host, key, slot, &info, result.as_ref())?,
        };
        assert_eq!(lines.len(), 1);
        let text = lines[0]
            .strip_prefix("text ")
            .unwrap()
            .strip_suffix(" 0 0|80")
            .unwrap();
        Ok(text.into())
    }
}

/// Lends rendering resources and releases the returned component and row after rendering.
fn render_component(
    host: &mut tests::host::Harness,
    key: u32,
    slot: &str,
    info: &Value,
    result: Option<&Value>,
) -> Result<Vec<String>, String> {
    use crate::bindings::host_side::maestro::extension::host as wire;
    use wasmtime::component::Resource;
    let state = host.store.data_mut();
    let theme = state
        .push(tests::renderer_host::Theme("dark".into()))
        .map_err(|e| e.to_string())?;
    let invalidator = state
        .push(tests::ui_host::Invalidator("paint".into()))
        .map_err(|e| e.to_string())?;
    let emitter = state
        .table
        .push(tests::bus_host::Emitter { key: 0 })
        .map_err(|e| e.to_string())?;
    let info = tests::ui_host::render_info(info);
    let render = wire::ToolRender {
        slot: if slot == "call" {
            wire::ToolRenderSlot::Call
        } else {
            wire::ToolRenderSlot::Result
        },
        row: None,
        args: info.args.clone(),
        result: result.map(ToString::to_string),
        options: Some(wire::ToolRenderResultOptions {
            expanded: info.expanded,
            is_partial: false,
        }),
        info,
    };
    let exports = host.exports.clone();
    let rendered = tests::block_on(exports.func_invoke_tool_render().call_async(
        &mut host.store,
        (
            tests::host::borrow(key),
            render,
            theme,
            invalidator,
            None,
            Resource::new_borrow(emitter.rep()),
        ),
    ))
    .map_err(|e| e.to_string())?
    .0?;
    component_lines(host, rendered.component?, rendered.row, emitter)
}

/// Renders and releases one returned component and its independently owned row.
fn component_lines(
    host: &mut tests::host::Harness,
    component: wasmtime::component::Resource<tests::host::Identity>,
    row: Option<wasmtime::component::Resource<tests::host::Identity>>,
    emitter: wasmtime::component::Resource<tests::bus_host::Emitter>,
) -> Result<Vec<String>, String> {
    use crate::bindings::host_side::maestro::extension::host as wire;
    use wasmtime::component::Resource;
    let exports = host.exports.clone();
    let lines = tests::block_on(exports.func_component_render().call_async(
        &mut host.store,
        (
            Resource::new_borrow(component.rep()),
            80,
            Resource::new_borrow(emitter.rep()),
        ),
    ))
    .map_err(|e| e.to_string())?
    .0?;
    for callback in [Some(component), row].into_iter().flatten() {
        tests::block_on(host.release(callback.rep())).map_err(|e| e.to_string())?;
        wire::HostCallback::drop(host.store.data_mut(), callback).map_err(|e| e.to_string())?;
    }
    host.store
        .data_mut()
        .table
        .delete(emitter)
        .map_err(|e| e.to_string())?;
    Ok(lines)
}
