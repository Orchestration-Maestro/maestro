//! Nested UI callbacks preserve their original retained capabilities.
use super::{Call, Callback, LocalCall, LocalResult};
use crate::guest_calls::paints::PaintCall;
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
    /// Original native row, transferred.
    #[component(name = "native-row")]
    native_row: Option<Resource<crate::native_renders::RowEndpoint>>,
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

/// Owned operands of one nested expansion callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalExpanded {
    /// Authored identity of the expansion handler.
    pub(super) target: u32,
    /// The requested expansion.
    pub(super) expanded: bool,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested authored styling.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalStyle {
    /// Authored identity of the style.
    target: u32,
    /// The text to style.
    text: String,
}

/// Owned operands of one nested authored highlighting.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalHighlight {
    /// Authored identity of the highlighting.
    target: u32,
    /// The code to highlight.
    code: String,
    /// The code block's language, when it names one.
    lang: Option<String>,
}

/// Owned operands of one nested authored widget callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(in super::super) struct LocalWidget {
    /// Authored identity of the callback.
    target: u32,
    /// What the widget tells it.
    event: crate::guest_calls::widget_calls::WireEvent,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Marshals one component factory, tool renderer, terminal input or widget callback.
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
            native_row: call
                .native_row
                .clone()
                .map(|row| table.push(row))
                .transpose()?,
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
        Call::Paint(PaintCall::Style(handler, text, _)) => LocalCall::Style(LocalStyle {
            target: handler.id,
            text: text.clone(),
        }),
        Call::Paint(PaintCall::Highlight(handler, code, lang, _)) => {
            LocalCall::Highlight(LocalHighlight {
                target: handler.id,
                code: code.clone(),
                lang: lang.clone(),
            })
        }
        Call::Widget(call) => LocalCall::Widget(LocalWidget {
            target: call.handler.id,
            event: call.event.lower(table)?,
            emitter: table.push(call.emitter.clone())?,
        }),
        _ => return Err(wasmtime::format_err!("expected a UI callback")),
    })
}

/// Delivers a nested factory, tool rendering, terminal input or widget callback's result.
pub(super) fn settle<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Call<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Call::TerminalInput(call), LocalResult::TerminalInput(value)) => {
            let _ = call.reply.send(value);
        }
        (Call::ToolRender(call), LocalResult::ToolRender(value)) => {
            let value = match value {
                Ok(rendered) => Ok(crate::guest_exports::rendered_owners(store, rendered)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Factory(call), LocalResult::Factory(value)) => {
            let value = match value {
                Ok(built) => Ok(crate::guest_exports::retained(store, built)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Paint(PaintCall::Style(_, _, reply)), LocalResult::Style(value)) => {
            let _ = reply.send(value);
        }
        (Call::Paint(PaintCall::Highlight(_, _, _, reply)), LocalResult::Highlight(value)) => {
            let _ = reply.send(value);
        }
        (Call::Widget(call), LocalResult::Widget(value)) => {
            let value = match value {
                Ok(answer) => Ok(crate::guest_calls::widget_calls::answered(store, answer)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        _ => {
            return Err(wasmtime::format_err!(
                "extension route result kind mismatch"
            ));
        }
    }
    Ok(())
}
