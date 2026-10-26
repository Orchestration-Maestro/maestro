//! Nested UI callbacks preserve their original retained capabilities.
use super::{Call, Callback, LocalCall};
use crate::local_resources::effects::EmitterEndpoint;
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Owned operands of one nested tool rendering.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalToolRender {
    /// Authored identity of the renderer.
    target: u32,
    /// The rendering operands.
    render: crate::guest_calls::tool_renders::Render,
    /// Original live theme, transferred.
    theme: Resource<crate::local_resources::effects::ThemeEndpoint>,
    /// The row's redraw capability, transferred.
    invalidator: Resource<crate::local_resources::effects::InvalidatorEndpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested component factory.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalFactory {
    /// Authored identity of the factory.
    target: u32,
    /// Original terminal, transferred.
    tui: Resource<crate::local_resources::effects::TuiEndpoint>,
    /// Original live theme, transferred.
    theme: Resource<crate::local_resources::effects::ThemeEndpoint>,
    /// Original footer metadata of a footer factory, transferred.
    footer: Option<Resource<crate::footer_data::Endpoint>>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested terminal input chunk.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalTerminalInput {
    /// Authored identity of the terminal input listener.
    target: u32,
    /// The literal chunk.
    data: String,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Marshals one component factory, tool renderer or terminal input callback.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Call<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::ToolRender(call) => LocalCall::ToolRender(LocalToolRender {
            target: call.handler.id,
            render: call.render.clone(),
            theme: table.push(call.theme.clone())?,
            invalidator: table.push(call.invalidator.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Factory(call) => LocalCall::Factory(LocalFactory {
            target: call.handler.id,
            tui: table.push(call.tui.clone())?,
            theme: table.push(call.theme.clone())?,
            footer: call
                .footer
                .clone()
                .map(|footer| table.push(footer))
                .transpose()?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::TerminalInput(call) => LocalCall::TerminalInput(LocalTerminalInput {
            target: call.handler.id,
            data: call.data.clone(),
            emitter: table.push(call.emitter.clone())?,
        }),
        _ => return Err(wasmtime::format_err!("expected a UI callback")),
    })
}
