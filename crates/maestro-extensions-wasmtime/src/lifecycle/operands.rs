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
            Self::OAuth(call) => Call::OAuth(oauth(call, completion)),
            Self::Provider(call) => Call::Provider(provider(call, completion)),
            Self::Assistant(call) => Call::Assistant(assistant(call, completion)),
            Self::ModelHook(call) => Call::ModelHook(model_hook(call, completion)),
            Self::Stream(call) => Call::Stream(stream(call, completion)),
            Self::Schema(call) => Call::Schema(schema(call, completion)),
            Self::Credential(call) => Call::Credential(credential(call, completion)),
            Self::Tool(call) => Call::Tool(tool(call, completion)),
            Self::Release(mut release) => {
                completion.retain(release.calls.take());
                Call::Release(release)
            }
            Self::Renderer(call) => Call::Renderer(renderer(call, completion)),
            Self::Event(call) => Call::Event(event(call, completion)),
            call @ (Self::Read(_)
            | Self::Write(_)
            | Self::Grep(_)
            | Self::Find(_)
            | Self::Ls(_)
            | Self::Edit(_)) => file_operation(call, completion),
            Self::Bash(call) => Call::Bash(crate::guest_calls::BashCall {
                owner: completion.callback(call.owner),
                request: call.request,
                output: call.output,
                signal: call.signal,
                reply: call.reply,
            }),
            call @ (Self::Factory(_)
            | Self::Custom(_)
            | Self::ToolRender(_)
            | Self::Paint(_)
            | Self::Widget(_)
            | Self::EditorFactory(_)
            | Self::AutocompleteFactory(_)
            | Self::Autocomplete(_)
            | Self::Editor(_)
            | Self::TerminalInput(_)) => ui(call, completion),
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

/// Transfers the admission of a UI callback beside its operands.
fn ui(call: Call, completion: &Completion) -> Call<Callback> {
    match call {
        Call::Factory(call) => Call::Factory(factory(call, completion)),
        Call::Custom(call) => Call::Custom(call.map(|handler| completion.callback(handler))),
        Call::ToolRender(call) => Call::ToolRender(tool_render(call, completion)),
        Call::Paint(call) => Call::Paint(call.map(|handler| completion.callback(handler))),
        Call::Widget(call) => Call::Widget(widget(call, completion)),
        Call::EditorFactory(call) => Call::EditorFactory(editor_factory(call, completion)),
        Call::AutocompleteFactory(call) => {
            Call::AutocompleteFactory(crate::guest_calls::providers::FactoryCall {
                handler: completion.callback(call.handler),
                current: call.current,
                emitter: call.emitter,
                reply: call.reply,
            })
        }
        Call::Autocomplete(call) => {
            Call::Autocomplete(crate::guest_calls::providers::ProviderCall {
                handler: completion.callback(call.handler),
                request: call.request,
                signal: call.signal,
                reply: call.reply,
            })
        }
        Call::Editor(call) => Call::Editor(crate::guest_calls::editors::EditorCall {
            handler: completion.callback(call.handler),
            request: call.request,
            emitter: call.emitter,
            reply: call.reply,
        }),
        Call::TerminalInput(call) => Call::TerminalInput(terminal_input(call, completion)),
        call => call.admitted(completion),
    }
}

/// Transfers the renderer admission beside its message operands.
fn renderer(call: RendererCall, completion: &Completion) -> RendererCall<Callback> {
    RendererCall {
        handler: completion.callback(call.handler),
        message: call.message,
        expanded: call.expanded,
        theme: call.theme,
        emitter: call.emitter,
        reply: call.reply,
    }
}

/// Transfers the event handler admission beside its context and signal.
fn event(
    call: crate::guest_calls::EventCall,
    completion: &Completion,
) -> crate::guest_calls::EventCall<Callback> {
    crate::guest_calls::EventCall {
        handler: completion.callback(call.handler),
        event: call.event,
        context: call.context,
        signal: call.signal,
        reply: call.reply,
    }
}

/// Transfers the widget callback admission beside its event.
fn widget(
    call: crate::guest_calls::widget_calls::WidgetCall,
    completion: &Completion,
) -> crate::guest_calls::widget_calls::WidgetCall<Callback> {
    crate::guest_calls::widget_calls::WidgetCall {
        handler: completion.callback(call.handler),
        event: call.event,
        emitter: call.emitter,
        reply: call.reply,
    }
}

/// Transfers the editor factory admission beside its lent terminal, styles and keybindings.
fn editor_factory(
    call: crate::guest_calls::editors::FactoryCall,
    completion: &Completion,
) -> crate::guest_calls::editors::FactoryCall<Callback> {
    crate::guest_calls::editors::FactoryCall {
        handler: completion.callback(call.handler),
        tui: call.tui,
        theme: call.theme,
        keybindings: call.keybindings,
        emitter: call.emitter,
        reply: call.reply,
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

/// Transfers the renderer admission beside its row operands.
fn tool_render(
    call: crate::guest_calls::tool_renders::ToolRenderCall,
    completion: &Completion,
) -> crate::guest_calls::tool_renders::ToolRenderCall<Callback> {
    crate::guest_calls::tool_renders::ToolRenderCall {
        handler: completion.callback(call.handler),
        render: call.render,
        theme: call.theme,
        invalidator: call.invalidator,
        native_row: call.native_row,
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
        ComponentCall::Focused(owner, emitter, reply) => {
            ComponentCall::Focused(completion.callback(owner), emitter, reply)
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

/// Transfers credential admissions without extending the native operand borrow.
fn credential(
    call: crate::guest_calls::credentials::CredentialCall,
    completion: &Completion,
) -> crate::guest_calls::credentials::CredentialCall<Callback> {
    use crate::guest_calls::credentials::{BackendCall, CredentialCall, FallbackCall};
    match call {
        CredentialCall::Backend(call) => CredentialCall::Backend(BackendCall {
            handler: completion.callback(call.handler),
            update: call.update,
            asynchronous: call.asynchronous,
            reply: call.reply,
        }),
        CredentialCall::Fallback(call) => CredentialCall::Fallback(FallbackCall {
            handler: completion.callback(call.handler),
            provider: call.provider,
            reply: call.reply,
        }),
    }
}

/// Retain the hook's original callback independently of its owned data operands.
fn model_hook(
    call: crate::model_calls::Invocation,
    completion: &Completion,
) -> crate::model_calls::Invocation<Callback> {
    crate::model_calls::Invocation {
        handler: completion.callback(call.handler),
        phase: call.phase,
        data: call.data,
        model: call.model,
        signal: call.signal,
        reply: call.reply,
    }
}

/// Retain both synchronous stream callback operands until the original invocation completes.
fn stream(
    call: crate::stream_calls::Invocation,
    completion: &Completion,
) -> crate::stream_calls::Invocation<Callback> {
    crate::stream_calls::Invocation {
        handler: completion.callback(call.handler),
        value: completion.callback(call.value),
        operation: call.operation,
        reply: call.reply,
    }
}

/// Transfer setup admission beside its original native capability operands.
fn provider(
    call: crate::provider_calls::Invocation,
    completion: &Completion,
) -> crate::provider_calls::Invocation<Callback> {
    crate::provider_calls::Invocation {
        handler: completion.callback(call.handler),
        data: call.data,
        input: call.input,
        reply: call.reply,
    }
}
/// Transfer returned-stream admission beside only its observation's cancellation.
fn assistant(
    call: crate::provider_calls::Assistant,
    completion: &Completion,
) -> crate::provider_calls::Assistant<Callback> {
    crate::provider_calls::Assistant {
        handler: completion.callback(call.handler),
        operation: call.operation,
        data: call.data,
        signal: call.signal,
        reply: call.reply,
    }
}

/// Transfer schema admission without extending its invocation-local operand borrow.
fn schema(
    call: crate::guest_calls::schema::Invocation,
    completion: &Completion,
) -> crate::guest_calls::schema::Invocation<Callback> {
    crate::guest_calls::schema::Invocation {
        handler: completion.callback(call.handler),
        operation: call.operation,
        node: call.node,
        emitter: call.emitter,
        reply: call.reply,
    }
}

/// Transfers the original provider or interaction callback with its native capability.
fn oauth(
    call: crate::oauth_callbacks::Invocation,
    completion: &Completion,
) -> crate::oauth_callbacks::Invocation<Callback> {
    crate::oauth_callbacks::Invocation {
        handler: completion.callback(call.handler),
        phase: call.phase,
        data: call.data,
        interaction: call.interaction,
        reply: call.reply,
    }
}

/// Transfers a reverse read's original callback admission beside its operands.
fn read(
    call: crate::guest_calls::read::Invocation,
    completion: &Completion,
) -> crate::guest_calls::read::Invocation<Callback> {
    crate::guest_calls::read::Invocation {
        handler: completion.callback(call.handler),
        operation: call.operation,
        path: call.path,
        reply: call.reply,
    }
}

/// Transfers the original grep observation beside its releasable admission.
fn grep(
    call: crate::guest_calls::grep::Invocation,
    completion: &Completion,
) -> crate::guest_calls::grep::Invocation<Callback> {
    crate::guest_calls::grep::Invocation {
        handler: completion.callback(call.handler),
        operation: call.operation,
        path: call.path,
        reply: call.reply,
    }
}
/// Transfers the original find observation beside its releasable admission.
fn find(
    call: crate::guest_calls::find::Invocation,
    completion: &Completion,
) -> crate::guest_calls::find::Invocation<Callback> {
    crate::guest_calls::find::Invocation {
        handler: completion.callback(call.handler),
        operation: call.operation,
        path: call.path,
        reply: call.reply,
    }
}

/// Transfers the original ls observation beside its releasable admission.
fn ls(
    call: crate::guest_calls::ls::Invocation,
    completion: &Completion,
) -> crate::guest_calls::ls::Invocation<Callback> {
    crate::guest_calls::ls::Invocation {
        handler: completion.callback(call.handler),
        operation: call.operation,
        path: call.path,
        reply: call.reply,
    }
}

/// Transfers the built-in file operation admissions beside their original operands.
fn file_operation(call: Call, completion: &Completion) -> Call<Callback> {
    match call {
        Call::Read(call) => Call::Read(read(call, completion)),
        Call::Write(call) => Call::Write(crate::guest_calls::write::Invocation {
            handler: completion.callback(call.handler),
            operation: call.operation,
            path: call.path,
            reply: call.reply,
        }),
        Call::Grep(call) => Call::Grep(grep(call, completion)),
        Call::Find(call) => Call::Find(find(call, completion)),
        Call::Ls(call) => Call::Ls(ls(call, completion)),
        Call::Edit(call) => Call::Edit(crate::guest_calls::edit::Invocation {
            handler: completion.callback(call.handler),
            operation: call.operation,
            path: call.path,
            reply: call.reply,
        }),
        call => call.admitted(completion),
    }
}
