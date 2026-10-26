//! Nested tool rendering transfers its original row and effect capabilities.
use super::{Callback, EmitterEndpoint};
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Owned operands of one nested tool rendering.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in crate::import_route) struct LocalToolRender {
    /// Authored identity of the renderer.
    target: u32,
    /// The rendering operands.
    render: crate::guest_calls::tool_renders::Render,
    /// Original live theme, transferred.
    theme: Resource<crate::local_resources::effects::ThemeEndpoint>,
    /// The row's redraw capability, transferred.
    invalidator: Resource<crate::local_resources::effects::InvalidatorEndpoint>,
    /// Original native row, transferred.
    #[component(name = "native-row")]
    native_row: Option<Resource<crate::native_renders::RowEndpoint>>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Transfers one nested tool renderer's original row and effect capabilities.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &crate::guest_calls::tool_renders::ToolRenderCall<Callback>,
) -> wasmtime::Result<LocalToolRender> {
    Ok(LocalToolRender {
        target: call.handler.id,
        render: call.render.clone(),
        theme: table.push(call.theme.clone())?,
        invalidator: table.push(call.invalidator.clone())?,
        native_row: call
            .native_row
            .clone()
            .map(|row| table.push(row))
            .transpose()?,
        emitter: table.push(call.emitter.clone())?,
    })
}
