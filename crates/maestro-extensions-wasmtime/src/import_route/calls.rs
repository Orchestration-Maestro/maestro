//! Typed nested same-instance calls, lowered from native calls and settled from guest results.
use crate::{
    context_imports::{CommandContext, Context},
    guest_calls::{
        BashRequest, Call, Callback, RegisteredCall, SynchronousKind,
        events::Returned,
        renderers::{ComponentCall, ComponentMethod},
        tools::ToolCall,
    },
    guest_exports::{
        bash::Capabilities as BashResources,
        events::Capabilities as EventResources,
        registrations::Item,
        tools::{Capabilities as ToolResources, Invocation as ToolInvocation},
    },
    local_resources::effects::{EmitterEndpoint, ThemeEndpoint},
};
use wasmtime::component::{ComponentType, Lift, Lower, Resource, ResourceTable};

mod credentials;
mod file_operations;
use file_operations::{LocalEdit, LocalRead, LocalWrite};
mod ui;
use ui::{LocalFactory, LocalTerminalInput, LocalToolRender};
mod tool;

/// Owned operands of one nested renderer call.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalRenderer {
    /// Authored identity in the shared callback inventory.
    target: u32,
    /// Complete custom-message document.
    message: String,
    /// Actual caller expansion choice.
    expanded: bool,
    /// Transferred theme capability.
    theme: Resource<ThemeEndpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested component render.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalRender {
    /// Authored identity of the retained component.
    target: u32,
    /// Actual caller viewport width.
    width: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested input or invalidation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalUnit {
    /// Authored identity of the retained component.
    target: u32,
    /// Literal input, or absence for invalidation.
    data: Option<String>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested key-release query.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalKey {
    /// Authored identity of the retained component.
    target: u32,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested argument preparation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalPrepare {
    /// Authored identity of the tool preparation callback.
    target: u32,
    /// Complete argument document.
    args: String,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested schema method on the still-borrowed native operand.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSchema {
    /// Authored identity of the retained schema node.
    target: u32,
    /// Exact author method requested by the checker.
    operation: crate::schema_wire::Operation,
    /// Invocation-local borrowed operand capability.
    node: Resource<crate::schema_reads::Endpoint>,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested synchronous callback prefix.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSynchronous {
    /// Authored identity of the callback.
    target: u32,
    /// Authored callback export.
    kind: SynchronousKind,
    /// Exact data document.
    data: String,
    /// Independent native invocation frame.
    emitter: Resource<EmitterEndpoint>,
}

/// Owned operands of one nested event callback.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalEvent {
    /// Authored identity of the event handler.
    target: u32,
    /// Complete tagged event document.
    event: String,
    /// Original invocation context and optional cancellation.
    resources: EventResources,
}

/// Owned operands of one nested command.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalCommand {
    /// Authored identity of the command handler.
    target: u32,
    /// Exact command arguments.
    args: String,
    /// Original command context.
    ctx: Resource<CommandContext>,
}

/// Owned operands of one nested argument completion.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalCompletion {
    /// Authored identity of the completion callback.
    target: u32,
    /// Exact argument prefix.
    prefix: String,
}

/// Owned operands of one nested shortcut.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalShortcut {
    /// Authored identity of the shortcut handler.
    target: u32,
    /// Original ordinary context.
    ctx: Resource<Context>,
}

/// Owned operands of one nested tool execution.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalTool {
    /// Authored identity of the tool execution callback.
    target: u32,
    /// Exact invocation identity and arguments.
    invocation: ToolInvocation,
    /// Original per-call context, cancellation and progress observer.
    resources: ToolResources,
}

/// Owned operands of one nested session setup.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalSetup {
    /// Authored identity of the setup callback.
    target: u32,
    /// Original writable session.
    writer: Resource<crate::writer_reads::Endpoint>,
}

/// Owned operands of one nested session continuation.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalContinuation {
    /// Authored identity of the continuation.
    target: u32,
    /// Original replacement context.
    ctx: Resource<crate::replaced_reads::Endpoint>,
}

/// Owned operands of one nested execution of returned shell operations.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalBash {
    /// Authored identity of the returned operations.
    target: u32,
    /// Exact execution operands.
    request: BashRequest,
    /// Original output frame and optional cancellation.
    resources: BashResources,
}

/// Owned operands of one nested credential backend method.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalBackend {
    /// Authored backend identity.
    target: u32,
    /// Invocation-local native update.
    update: Resource<crate::local_resources::credential_scope::Endpoint>,
    /// Whether the author must await its update.
    asynchronous: bool,
}
/// Owned operands of one nested fallback lookup.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(super) struct LocalFallback {
    /// Authored resolver identity.
    target: u32,
    /// Literal provider.
    provider: String,
}
/// The nested same-instance target and its owned operands.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(super) enum LocalCall {
    /// Authored backend and still-borrowed native update.
    #[component(name = "auth-backend")]
    AuthBackend(LocalBackend),
    /// Authored fallback with its original optional result.
    #[component(name = "auth-fallback")]
    AuthFallback(LocalFallback),
    /// A message renderer.
    #[component(name = "renderer")]
    Renderer(LocalRenderer),
    /// A retained component render.
    #[component(name = "render")]
    Render(LocalRender),
    /// A retained component input or invalidation.
    #[component(name = "unit")]
    Unit(LocalUnit),
    /// A retained component key-release query.
    #[component(name = "key")]
    Key(LocalKey),
    /// Tool argument preparation.
    #[component(name = "prepare")]
    Prepare(LocalPrepare),
    /// A terminal input listener.
    #[component(name = "terminal-input")]
    TerminalInput(LocalTerminalInput),
    /// A synchronous callback prefix.
    #[component(name = "synchronous")]
    Synchronous(LocalSynchronous),
    /// A schema method against the original borrowed operand.
    #[component(name = "schema")]
    Schema(LocalSchema),
    /// A synchronous authored-stream classifier or extractor.
    #[component(name = "stream")]
    Stream(crate::stream_calls::Local),
    /// An event callback.
    #[component(name = "event")]
    Event(LocalEvent),
    /// A command handler.
    #[component(name = "command")]
    Command(LocalCommand),
    /// An argument-completion callback.
    #[component(name = "completion")]
    Completion(LocalCompletion),
    /// A shortcut handler.
    #[component(name = "shortcut")]
    Shortcut(LocalShortcut),
    /// A tool execution.
    #[component(name = "tool")]
    Tool(LocalTool),
    /// A new-session setup.
    #[component(name = "setup")]
    Setup(LocalSetup),
    /// A session continuation.
    #[component(name = "with-session")]
    WithSession(LocalContinuation),
    /// An independently owned listener tail.
    #[component(name = "tail")]
    Tail(u32),
    /// An execution of returned shell operations.
    #[component(name = "bash")]
    Bash(LocalBash),
    /// A component factory.
    #[component(name = "factory")]
    Factory(LocalFactory),
    /// A tool rendering.
    #[component(name = "tool-render")]
    ToolRender(LocalToolRender),
    /// A reverse read operation.
    #[component(name = "read")]
    Read(LocalRead),
    /// A reverse write effect.
    #[component(name = "write")]
    Write(LocalWrite),
    /// A reverse edit effect.
    #[component(name = "edit")]
    Edit(LocalEdit),
}

/// The nested call's own result, returned to the native caller that asked for it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(super) enum LocalResult {
    /// Exact optional fallback value or authored error.
    #[component(name = "auth-fallback")]
    AuthFallback(Result<Option<String>, String>),
    /// Optional retained component identity, or the authored error.
    #[component(name = "renderer")]
    Renderer(Result<Option<Resource<Callback>>, String>),
    /// Exact rendering lines or the authored error.
    #[component(name = "render")]
    Render(Result<Vec<String>, String>),
    /// Unit result or the authored error.
    #[component(name = "unit")]
    Unit(Result<(), String>),
    /// Key-release preference or the authored error.
    #[component(name = "key")]
    Key(Result<bool, String>),
    /// Prepared argument document or the authored error.
    #[component(name = "prepared")]
    Prepared(Result<String, String>),
    /// The terminal input listener's decision or its error.
    #[component(name = "terminal-input")]
    TerminalInput(Result<Option<crate::guest_calls::terminal::Decision>, String>),
    /// Unit result of a synchronous prefix, or the authored error.
    #[component(name = "synchronous")]
    Synchronous(Result<(), String>),
    /// The schema result with every authored diagnostic field.
    #[component(name = "schema")]
    Schema(Result<String, crate::schema_wire::Error>),
    /// Selected terminal classification or retained result.
    #[component(name = "stream")]
    Stream(Result<crate::stream_calls::WireSelection, String>),
    /// Event outcome, or the message rejecting the event before its handler ran.
    #[component(name = "event")]
    Event(Result<Returned, String>),
    /// Optional ordered completion candidates, or the authored error.
    #[component(name = "completion")]
    Completion(Result<Option<Vec<Item>>, String>),
    /// Tool result document, or the authored error.
    #[component(name = "tool")]
    Tool(Result<String, String>),
    /// Optional exit status, or the authored error.
    #[component(name = "bash")]
    Bash(Result<Option<f64>, String>),
    /// The built component identities, or the authored error.
    #[component(name = "factory")]
    Factory(Result<crate::guest_calls::factories::Built, String>),
    /// The rendered component and new row state identities, or the authored error.
    #[component(name = "tool-render")]
    ToolRender(Result<crate::guest_calls::tool_renders::Rendered, String>),
    /// Original read observation or structured diagnostic.
    #[component(name = "read")]
    Read(Result<crate::guest_calls::read::Output, crate::schema_wire::Error>),
    /// Original write completion or structured diagnostic.
    #[component(name = "write")]
    Write(Result<(), crate::schema_wire::Error>),
    /// Original edit observation or structured diagnostic.
    #[component(name = "edit")]
    Edit(Result<crate::guest_calls::read::Output, crate::schema_wire::Error>),
}

/// Marshals owned operands without entering a new canonical guest activation.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Call<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::Credential(call) => credentials::lower(table, call)?,
        Call::Renderer(call) => LocalCall::Renderer(LocalRenderer {
            target: call.handler.id,
            message: call.message.clone(),
            expanded: call.expanded,
            theme: table.push(call.theme.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Component(call) => component(table, call)?,
        Call::Tool(ToolCall::Prepare {
            handler,
            args,
            emitter,
            ..
        }) => LocalCall::Prepare(LocalPrepare {
            target: handler.id,
            args: args.clone(),
            emitter: table.push(emitter.clone())?,
        }),
        Call::Stream(call) => LocalCall::Stream(crate::stream_calls::Local::new(call)),
        Call::Schema(call) => LocalCall::Schema(LocalSchema {
            target: call.handler.id,
            operation: call.operation.clone(),
            node: table.push(call.node.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
        call @ (Call::ToolRender(_) | Call::Factory(_) | Call::TerminalInput(_)) => {
            ui::lower(table, call)?
        }
        Call::Synchronous(call) => LocalCall::Synchronous(LocalSynchronous {
            target: call.handler.id,
            kind: call.kind,
            data: call.data.clone(),
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Registered(call) => registered(table, call)?,
        call => asynchronous(table, call)?,
    })
}

/// Marshals one retained component method.
fn component(
    table: &mut ResourceTable,
    call: &ComponentCall<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        ComponentCall::Render(owner, width, emitter, _) => LocalCall::Render(LocalRender {
            target: owner.id,
            width: *width,
            emitter: table.push(emitter.clone())?,
        }),
        ComponentCall::Unit(owner, method, emitter, _) => LocalCall::Unit(LocalUnit {
            target: owner.id,
            data: match method {
                ComponentMethod::Input(data) => Some(data.clone()),
                ComponentMethod::Invalidate => None,
            },
            emitter: table.push(emitter.clone())?,
        }),
        ComponentCall::WantsKeyRelease(owner, emitter, _) => LocalCall::Key(LocalKey {
            target: owner.id,
            emitter: table.push(emitter.clone())?,
        }),
    })
}

/// Marshals one registered command, completion or shortcut.
fn registered(
    table: &mut ResourceTable,
    call: &RegisteredCall<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        RegisteredCall::Command(handler, args, context, _) => LocalCall::Command(LocalCommand {
            target: handler.id,
            args: args.clone(),
            ctx: table.push(CommandContext(context.clone()))?,
        }),
        RegisteredCall::Completion(handler, prefix, _) => LocalCall::Completion(LocalCompletion {
            target: handler.id,
            prefix: prefix.clone(),
        }),
        RegisteredCall::Shortcut(handler, context, _) => LocalCall::Shortcut(LocalShortcut {
            target: handler.id,
            ctx: table.push(Context(context.clone()))?,
        }),
    })
}

/// Marshals the remaining asynchronous callbacks with their original capabilities.
fn asynchronous(table: &mut ResourceTable, call: &Call<Callback>) -> wasmtime::Result<LocalCall> {
    Ok(match call {
        Call::Edit(call) => LocalCall::Edit(LocalEdit {
            target: call.handler.id,
            operation: call.operation.clone(),
            path: call.path.clone(),
        }),
        Call::Write(call) => LocalCall::Write(LocalWrite {
            target: call.handler.id,
            operation: call.operation.clone(),
            path: call.path.clone(),
        }),
        Call::Read(call) => LocalCall::Read(LocalRead {
            target: call.handler.id,
            operation: call.operation,
            path: call.path.clone(),
        }),
        Call::Event(call) => LocalCall::Event(LocalEvent {
            target: call.handler.id,
            event: call.event.clone(),
            resources: EventResources {
                ctx: table.push(Context(call.context.clone()))?,
                signal: call
                    .signal
                    .clone()
                    .map(|signal| table.push(signal))
                    .transpose()?,
            },
        }),
        Call::Tool(call) => tool::lower(table, call)?,
        Call::Setup(handler, writer, _) => LocalCall::Setup(LocalSetup {
            target: handler.id,
            writer: table.push(writer.clone())?,
        }),
        Call::WithSession(handler, context, _) => LocalCall::WithSession(LocalContinuation {
            target: handler.id,
            ctx: table.push(context.clone())?,
        }),
        Call::Tail(handler, _) => LocalCall::Tail(handler.id),
        Call::Bash(call) => LocalCall::Bash(LocalBash {
            target: call.owner.id,
            request: call.request.clone(),
            resources: BashResources {
                output: table.push(call.output.clone())?,
                signal: call
                    .signal
                    .clone()
                    .map(|signal| table.push(signal))
                    .transpose()?,
            },
        }),
        _ => return Err(wasmtime::format_err!("extension call cannot run nested")),
    })
}

/// Delivers a nested call's own result to the native caller that asked for it.
pub(super) fn settle<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Call<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Call::Credential(call), value) => credentials::settle(call, value)?,
        (Call::Renderer(call), LocalResult::Renderer(value)) => {
            let value = match value {
                Ok(component) => Ok(component
                    .map(|component| crate::callback_imports::transferred(store, component))
                    .transpose()?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Component(ComponentCall::Render(_, _, _, reply)), LocalResult::Render(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::Unit(_, _, _, reply)), LocalResult::Unit(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::WantsKeyRelease(_, _, reply)), LocalResult::Key(value)) => {
            let _ = reply.send(value);
        }
        (Call::Tool(ToolCall::Prepare { reply, .. }), LocalResult::Prepared(value)) => {
            let _ = reply.send(value);
        }
        (Call::Stream(call), LocalResult::Stream(value)) => {
            let value = match value {
                Ok(value) => Ok(value.retained(store)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Schema(call), LocalResult::Schema(value)) => {
            let _ = call.reply.send(Ok(value));
        }
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
        (Call::Synchronous(call), LocalResult::Synchronous(value)) => {
            let _ = call.completion.send(value);
        }
        (call, value) => settle_asynchronous(store, call, value)?,
    }
    Ok(())
}

/// Delivers the result of a nested asynchronous callback.
fn settle_asynchronous<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Call<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Call::Event(call), LocalResult::Event(value)) => {
            let value = match value {
                Ok(returned) => Ok(returned.retained(store)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (
            Call::Registered(
                RegisteredCall::Command(_, _, _, reply) | RegisteredCall::Shortcut(_, _, reply),
            )
            | Call::Setup(_, _, reply)
            | Call::WithSession(_, _, reply)
            | Call::Tail(_, reply),
            LocalResult::Unit(value),
        ) => {
            let _ = reply.send(value);
        }
        (
            Call::Registered(RegisteredCall::Completion(_, _, reply)),
            LocalResult::Completion(value),
        ) => {
            let value =
                value.map(|items| items.map(|items| items.into_iter().map(Into::into).collect()));
            let _ = reply.send(value);
        }
        (Call::Tool(ToolCall::Execute { reply, .. }), LocalResult::Tool(value)) => {
            let _ = reply.send(value);
        }
        (Call::Edit(call), LocalResult::Edit(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Write(call), LocalResult::Write(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Read(call), LocalResult::Read(value)) => {
            let _ = call.reply.send(Ok(value.map_err(Into::into)));
        }
        (Call::Bash(call), LocalResult::Bash(value)) => {
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
