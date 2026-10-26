//! Owned operand records of nested same-instance calls.
use crate::{
    context_imports::{CommandContext, Context},
    guest_calls::{BashRequest, SynchronousKind},
    guest_exports::{
        bash::Capabilities as BashResources,
        events::Capabilities as EventResources,
        tools::{Capabilities as ToolResources, Invocation as ToolInvocation},
    },
    local_resources::effects::{EmitterEndpoint, ThemeEndpoint},
};
use wasmtime::component::{ComponentType, Lower, Resource};

/// Owned operands of one nested renderer call.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalRenderer {
    /// Authored identity in the shared callback inventory.
    pub(super) target: u32,
    /// Complete custom-message document.
    pub(super) message: String,
    /// Actual caller expansion choice.
    pub(super) expanded: bool,
    /// Transferred theme capability.
    pub(super) theme: Resource<ThemeEndpoint>,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested component render.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalRender {
    /// Authored identity of the retained component.
    pub(super) target: u32,
    /// Actual caller viewport width.
    pub(super) width: u32,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested input or invalidation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalUnit {
    /// Authored identity of the retained component.
    pub(super) target: u32,
    /// Literal input, or absence for invalidation.
    pub(super) data: Option<String>,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested key-release query.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalKey {
    /// Authored identity of the retained component.
    pub(super) target: u32,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested tool rendering.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalToolRender {
    /// Authored identity of the renderer.
    pub(super) target: u32,
    /// The rendering operands.
    pub(super) render: crate::guest_calls::tool_renders::Render,
    /// Original live theme, transferred.
    pub(super) theme: Resource<crate::local_resources::effects::ThemeEndpoint>,
    /// The row's redraw capability, transferred.
    pub(super) invalidator: Resource<crate::local_resources::effects::InvalidatorEndpoint>,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested component factory.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalFactory {
    /// Authored identity of the factory.
    pub(super) target: u32,
    /// Original terminal, transferred.
    pub(super) tui: Resource<crate::local_resources::effects::TuiEndpoint>,
    /// Original live theme, transferred.
    pub(super) theme: Resource<crate::local_resources::effects::ThemeEndpoint>,
    /// Original footer metadata of a footer factory, transferred.
    pub(super) footer: Option<Resource<crate::footer_data::Endpoint>>,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested terminal input chunk.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalTerminalInput {
    /// Authored identity of the terminal input listener.
    pub(super) target: u32,
    /// The literal chunk.
    pub(super) data: String,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested argument preparation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalPrepare {
    /// Authored identity of the tool preparation callback.
    pub(super) target: u32,
    /// Complete argument document.
    pub(super) args: String,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested schema method on the still-borrowed native operand.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSchema {
    /// Authored identity of the retained schema node.
    pub(super) target: u32,
    /// Exact author method requested by the checker.
    pub(super) operation: crate::schema_wire::Operation,
    /// Invocation-local borrowed operand capability.
    pub(super) node: Resource<crate::schema_reads::Endpoint>,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested synchronous callback prefix.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSynchronous {
    /// Authored identity of the callback.
    pub(super) target: u32,
    /// Authored callback export.
    pub(super) kind: SynchronousKind,
    /// Exact data document.
    pub(super) data: String,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested event callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalEvent {
    /// Authored identity of the event handler.
    pub(super) target: u32,
    /// Complete tagged event document.
    pub(super) event: String,
    /// Original invocation context and optional cancellation.
    pub(super) resources: EventResources,
}

/// Owned operands of one nested command.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalCommand {
    /// Authored identity of the command handler.
    pub(super) target: u32,
    /// Exact command arguments.
    pub(super) args: String,
    /// Original command context.
    pub(super) ctx: Resource<CommandContext>,
}

/// Owned operands of one nested argument completion.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalCompletion {
    /// Authored identity of the completion callback.
    pub(super) target: u32,
    /// Exact argument prefix.
    pub(super) prefix: String,
}

/// Owned operands of one nested shortcut.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalShortcut {
    /// Authored identity of the shortcut handler.
    pub(super) target: u32,
    /// Original ordinary context.
    pub(super) ctx: Resource<Context>,
}

/// Owned operands of one nested tool execution.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalTool {
    /// Authored identity of the tool execution callback.
    pub(super) target: u32,
    /// Exact invocation identity and arguments.
    pub(super) invocation: ToolInvocation,
    /// Original per-call context, cancellation and progress observer.
    pub(super) resources: ToolResources,
}

/// Owned operands of one nested session setup.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSetup {
    /// Authored identity of the setup callback.
    pub(super) target: u32,
    /// Original writable session.
    pub(super) writer: Resource<crate::writer_reads::Endpoint>,
}

/// Owned operands of one nested session continuation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalContinuation {
    /// Authored identity of the continuation.
    pub(super) target: u32,
    /// Original replacement context.
    pub(super) ctx: Resource<crate::replaced_reads::Endpoint>,
}

/// Owned operands of one nested execution of returned shell operations.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalBash {
    /// Authored identity of the returned operations.
    pub(super) target: u32,
    /// Exact execution operands.
    pub(super) request: BashRequest,
    /// Original output frame and optional cancellation.
    pub(super) resources: BashResources,
}

/// Owned operands of one nested authored styling.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalStyle {
    /// Authored identity of the style.
    pub(super) target: u32,
    /// The text to style.
    pub(super) text: String,
}

/// Owned operands of one nested authored highlighting.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalHighlight {
    /// Authored identity of the highlighting.
    pub(super) target: u32,
    /// The code to highlight.
    pub(super) code: String,
    /// The code block's language, when it names one.
    pub(super) lang: Option<String>,
}

/// Owned operands of one nested authored widget callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalWidget {
    /// Authored identity of the callback.
    pub(super) target: u32,
    /// What the widget tells it.
    pub(super) event: crate::guest_calls::widget_calls::WireEvent,
    /// Independent native invocation frame.
    pub(super) emitter: Resource<EmitterEndpoint>,
}
