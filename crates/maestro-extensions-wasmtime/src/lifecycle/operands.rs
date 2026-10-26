//! Separates typed guest operands from the invocation's releasable admission owners.
use super::completion::Completion;
use crate::guest_calls::{
    Call, Callback,
    registrations::RegisteredCall,
    renderers::{ComponentCall, RendererCall},
    tools::ToolCall,
};

impl Call {
    /// Transfers every admitting operand before Wasmtime can retain the task.
    pub(crate) fn admitted(self, completion: &Completion) -> Call<Callback> {
        match self {
            Self::Start(reply) => Call::Start(reply),
            Self::Finished => Call::Finished,
            Self::Setup(handler, writer, reply) => {
                Call::Setup(completion.callback(handler), writer, reply)
            }
            Self::WithSession(handler, context, reply) => {
                Call::WithSession(completion.callback(handler), context, reply)
            }
            Self::Tail(handler, reply) => Call::Tail(completion.callback(handler), reply),
            Self::Registered(call) => Call::Registered(registered(call, completion)),
            Self::Component(call) => Call::Component(component(call, completion)),
            Self::Schema(call) => Call::Schema(crate::guest_calls::schema::Invocation {
                handler: completion.callback(call.handler),
                operation: call.operation,
                node: call.node,
                emitter: call.emitter,
                reply: call.reply,
            }),
            Self::Tool(call) => Call::Tool(tool(call, completion)),
            Self::Release(mut release) => {
                completion.retain(release.calls.take());
                Call::Release(release)
            }
            Self::Renderer(call) => Call::Renderer(RendererCall {
                handler: completion.callback(call.handler),
                message: call.message,
                expanded: call.expanded,
                theme: call.theme,
                emitter: call.emitter,
                reply: call.reply,
            }),
            Self::Event(call) => Call::Event(crate::guest_calls::EventCall {
                handler: completion.callback(call.handler),
                event: call.event,
                context: call.context,
                signal: call.signal,
                reply: call.reply,
            }),
            Self::Bash(call) => Call::Bash(crate::guest_calls::BashCall {
                owner: completion.callback(call.owner),
                request: call.request,
                output: call.output,
                signal: call.signal,
                reply: call.reply,
            }),
            Self::Factory(call) => Call::Factory(factory(call, completion)),
            Self::TerminalInput(call) => Call::TerminalInput(terminal_input(call, completion)),
            Self::Synchronous(call) => Call::Synchronous(crate::guest_calls::Synchronous {
                handler: completion.callback(call.handler),
                kind: call.kind,
                data: call.data,
                emitter: call.emitter,
                completion: call.completion,
            }),
        }
    }
}

/// Transfers the factory admission beside its lent terminal and theme.
fn factory(
    call: crate::guest_calls::factories::FactoryCall,
    completion: &Completion,
) -> crate::guest_calls::factories::FactoryCall<Callback> {
    crate::guest_calls::factories::FactoryCall {
        handler: completion.callback(call.handler),
        tui: call.tui,
        theme: call.theme,
        footer: call.footer,
        emitter: call.emitter,
        reply: call.reply,
    }
}

/// Transfers the terminal input listener admission beside its chunk.
fn terminal_input(
    call: crate::guest_calls::terminal::Input,
    completion: &Completion,
) -> crate::guest_calls::terminal::Input<Callback> {
    crate::guest_calls::terminal::Input {
        handler: completion.callback(call.handler),
        data: call.data,
        emitter: call.emitter,
        reply: call.reply,
    }
}

/// Transfers callback ownership without changing any registration operands.
fn registered(call: RegisteredCall, completion: &Completion) -> RegisteredCall<Callback> {
    match call {
        RegisteredCall::Command(handler, args, context, reply) => {
            RegisteredCall::Command(completion.callback(handler), args, context, reply)
        }
        RegisteredCall::Completion(handler, prefix, reply) => {
            RegisteredCall::Completion(completion.callback(handler), prefix, reply)
        }
        RegisteredCall::Shortcut(handler, context, reply) => {
            RegisteredCall::Shortcut(completion.callback(handler), context, reply)
        }
    }
}

/// Transfers callback ownership while preserving each component method.
fn component(call: ComponentCall, completion: &Completion) -> ComponentCall<Callback> {
    match call {
        ComponentCall::Render(owner, width, emitter, reply) => {
            ComponentCall::Render(completion.callback(owner), width, emitter, reply)
        }
        ComponentCall::Unit(owner, method, emitter, reply) => {
            ComponentCall::Unit(completion.callback(owner), method, emitter, reply)
        }
        ComponentCall::WantsKeyRelease(owner, emitter, reply) => {
            ComponentCall::WantsKeyRelease(completion.callback(owner), emitter, reply)
        }
    }
}

/// Transfers the native tool admission independently of its caller capabilities.
fn tool(call: ToolCall, completion: &Completion) -> ToolCall<Callback> {
    match call {
        ToolCall::Prepare {
            handler,
            args,
            emitter,
            reply,
        } => ToolCall::Prepare {
            handler: completion.callback(handler),
            args,
            emitter,
            reply,
        },
        ToolCall::Execute {
            handler,
            id,
            params,
            context,
            signal,
            update,
            reply,
        } => ToolCall::Execute {
            handler: completion.callback(handler),
            id,
            params,
            context,
            signal,
            update,
            reply,
        },
    }
}
