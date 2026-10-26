//! Typed nested same-instance calls, lowered from native calls and settled from guest results.
use super::records::{
    LocalBash, LocalCommand, LocalCompletion, LocalContinuation, LocalEvent, LocalExpanded,
    LocalFactory, LocalHighlight, LocalKey, LocalPrepare, LocalRender, LocalRenderer, LocalSchema,
    LocalSetup, LocalShortcut, LocalStyle, LocalSynchronous, LocalTerminalInput, LocalTool,
    LocalToolRender, LocalUnit, LocalWidget,
};
use crate::{
    context_imports::{CommandContext, Context},
    guest_calls::{
        Call, Callback, RegisteredCall,
        events::Returned,
        paints::PaintCall,
        renderers::{ComponentCall, ComponentMethod},
        tools::ToolCall,
    },
    guest_exports::{
        bash::Capabilities as BashResources,
        events::Capabilities as EventResources,
        registrations::Item,
        tools::{Capabilities as ToolResources, Invocation as ToolInvocation},
    },
};
use wasmtime::component::{ComponentType, Lift, Lower, Resource, ResourceTable};

/// The nested same-instance target and its owned operands.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(super) enum LocalCall {
    /// A message renderer.
    #[component(name = "renderer")]
    Renderer(LocalRenderer),
    /// A retained component render.
    #[component(name = "render")]
    Render(LocalRender),
    /// A retained component input or invalidation.
    #[component(name = "unit")]
    Unit(LocalUnit),
    /// A retained expansion callback.
    #[component(name = "expanded")]
    Expanded(LocalExpanded),
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
    /// An authored styling.
    #[component(name = "style")]
    Style(LocalStyle),
    /// An authored highlighting.
    #[component(name = "highlight")]
    Highlight(LocalHighlight),
    /// An authored widget callback.
    #[component(name = "widget")]
    Widget(LocalWidget),
}

/// The nested call's own result, returned to the native caller that asked for it.
#[derive(ComponentType, Lift)]
#[component(variant)]
pub(super) enum LocalResult {
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
    /// The styled text, or the authored error.
    #[component(name = "style")]
    Style(Result<String, String>),
    /// The highlighted rows, or the authored error.
    #[component(name = "highlight")]
    Highlight(Result<Vec<String>, String>),
    /// The widget callback's answer, or the authored error.
    #[component(name = "widget")]
    Widget(Result<crate::guest_calls::widget_calls::Answer, String>),
}

/// Marshals owned operands without entering a new canonical guest activation.
pub(super) fn lower(
    table: &mut ResourceTable,
    call: &Call<Callback>,
) -> wasmtime::Result<LocalCall> {
    Ok(match call {
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
        Call::Schema(call) => LocalCall::Schema(LocalSchema {
            target: call.handler.id,
            operation: call.operation.clone(),
            node: table.push(call.node.clone())?,
            emitter: table.push(call.emitter.clone())?,
        }),
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
        Call::Synchronous(call) => LocalCall::Synchronous(LocalSynchronous {
            target: call.handler.id,
            kind: call.kind,
            data: call.data.clone(),
            emitter: table.push(call.emitter.clone())?,
        }),
        Call::Registered(call) => registered(table, call)?,
        call @ (Call::Paint(_) | Call::Widget(_)) => paint(table, call)?,
        call => asynchronous(table, call)?,
    })
}

/// Marshals one authored styling, highlighting or widget callback.
fn paint(table: &mut ResourceTable, call: &Call<Callback>) -> wasmtime::Result<LocalCall> {
    Ok(match call {
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
        _ => return Err(wasmtime::format_err!("expected a widget call")),
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
        ComponentCall::Unit(owner, method, emitter, _) => {
            let target = owner.id;
            let emitter = table.push(emitter.clone())?;
            match method {
                ComponentMethod::SetExpanded(expanded) => LocalCall::Expanded(LocalExpanded {
                    target,
                    expanded: *expanded,
                    emitter,
                }),
                ComponentMethod::Input(data) => LocalCall::Unit(LocalUnit {
                    target,
                    data: Some(data.clone()),
                    emitter,
                }),
                ComponentMethod::Invalidate => LocalCall::Unit(LocalUnit {
                    target,
                    data: None,
                    emitter,
                }),
            }
        }
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
        Call::Tool(call) => tool(table, call)?,
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

/// Marshals one tool execution with its per-call capabilities.
fn tool(table: &mut ResourceTable, call: &ToolCall<Callback>) -> wasmtime::Result<LocalCall> {
    let ToolCall::Execute {
        handler,
        id,
        params,
        context,
        signal,
        update,
        ..
    } = call
    else {
        return Err(wasmtime::format_err!(
            "extension preparation is synchronous"
        ));
    };
    Ok(LocalCall::Tool(LocalTool {
        target: handler.id,
        invocation: ToolInvocation {
            tool_call_id: id.clone(),
            params: params.clone(),
        },
        resources: ToolResources {
            ctx: table.push(Context(context.clone()))?,
            signal: signal
                .clone()
                .map(|signal| table.push(signal))
                .transpose()?,
            update: update
                .clone()
                .map(|update| table.push(update))
                .transpose()?,
        },
    }))
}

/// Delivers a nested call's own result to the native caller that asked for it.
pub(super) fn settle<T: crate::callback_imports::CallbackView>(
    store: &mut T,
    call: Call<Callback>,
    value: LocalResult,
) -> wasmtime::Result<()> {
    match (call, value) {
        (Call::Renderer(call), LocalResult::Renderer(value)) => {
            let value = match value {
                Ok(component) => Ok(component
                    .map(|component| crate::callback_imports::transferred(store, component))
                    .transpose()?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
        }
        (Call::Component(ComponentCall::Render(_, _, _, reply)), LocalResult::Render(value))
        | (Call::Paint(PaintCall::Highlight(_, _, _, reply)), LocalResult::Highlight(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::Unit(_, _, _, reply)), LocalResult::Unit(value)) => {
            let _ = reply.send(value);
        }
        (Call::Component(ComponentCall::WantsKeyRelease(_, _, reply)), LocalResult::Key(value)) => {
            let _ = reply.send(value);
        }
        (Call::Tool(ToolCall::Prepare { reply, .. }), LocalResult::Prepared(value))
        | (Call::Paint(PaintCall::Style(_, _, reply)), LocalResult::Style(value)) => {
            let _ = reply.send(value);
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
        (Call::Widget(call), LocalResult::Widget(value)) => {
            let value = match value {
                Ok(answer) => Ok(crate::guest_calls::widget_calls::answered(store, answer)?),
                Err(error) => Err(error),
            };
            let _ = call.reply.send(value);
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
