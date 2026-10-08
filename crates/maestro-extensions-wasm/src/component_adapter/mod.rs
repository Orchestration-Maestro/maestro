//! Component adapter: forwards registrations to the generated imports and serves the
//! generated exports from the private closure table.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod callbacks;
mod contexts;
mod host_api;
mod session;

use std::marker::PhantomData;
use std::rc::Rc;

use crate::bindings::exports::maestro::extension::guest::{
    self, Guest, GuestBashOperations, GuestComponent,
};
use crate::bindings::maestro::extension::events as wire;
use crate::bindings::maestro::extension::host;
use crate::bindings::maestro::extension::models as wire_models;
use crate::bindings::maestro::extension::session as wire_session;
use crate::event_bus::{CallbackEmitter, EventListener};
use crate::loader::Extension;
use crate::tui::Component;
use crate::types::{
    ArgumentCompletions, BashOperations, CommandHandler, CompactionComplete, CompactionError,
    ExtensionAPI, ExtensionEvent, ExtensionHandler, MessageRenderer, PrepareArguments,
    SessionEvent, SetupSession, ShortcutHandler, ToolExecute, WithSession,
};

/// Entry type of an extension component: serves the generated exports for `E`.
#[doc(hidden)]
pub struct Glue<E>(PhantomData<E>);

/// Exported resource that retains the operations an extension supplied to the host.
pub struct ExportedBashOperations {
    /// The retained operations.
    _operations: BashOperations,
}

impl GuestBashOperations for ExportedBashOperations {}

/// Exported component resource wrapping the view an extension handed to the host.
pub struct ExportedComponent {
    /// The component.
    component: Component,
}

impl GuestComponent for ExportedComponent {
    fn render(&self, width: u32, emitter: &host::CallbackEmitter) -> Result<Vec<String>, String> {
        self.component.render(
            width,
            CallbackEmitter::new(&contexts::GeneratedEmitter(emitter)),
        )
    }

    fn handle_input(&self, data: String, emitter: &host::CallbackEmitter) -> Result<(), String> {
        self.component.handle_input(
            &data,
            CallbackEmitter::new(&contexts::GeneratedEmitter(emitter)),
        )
    }

    fn wants_key_release(&self) -> bool {
        self.component.wants_key_release()
    }

    fn invalidate(&self, emitter: &host::CallbackEmitter) -> Result<(), String> {
        self.component
            .invalidate(CallbackEmitter::new(&contexts::GeneratedEmitter(emitter)))
    }
}

/// Runs the handler registered under a host identity against an event the caller keeps, so
/// the caller sees the edits the handler made even when it failed.
async fn run(
    handler: &host::Callback,
    event: &mut ExtensionEvent,
    ctx: host::Context,
) -> (guest::Decision, Option<BashOperations>) {
    let handler = match callbacks::find::<ExtensionHandler>(handler) {
        Ok(handler) => handler,
        Err(message) => return (Err(message), None),
    };
    match handler(event, contexts::context(ctx)).await {
        Ok(Some(result)) => {
            let (result, operations) = result.into_wire();
            (Ok(Some(result)), operations)
        }
        Ok(None) => (Ok(None), None),
        Err(message) => (Err(message), None),
    }
}

/// Declares the exports that run a handler against an event without returning the event.
macro_rules! event_exports {
    ($($method:ident($payload:ty) => $build:expr;)*) => {
        $(
            async fn $method(
                handler: &host::Callback,
                event: $payload,
                ctx: host::Context,
            ) -> guest::Decision {
                run(handler, &mut $build(event), ctx).await.0
            }
        )*
    };
}

impl<E: Extension> Guest for Glue<E> {
    type BashOperations = ExportedBashOperations;
    type Component = ExportedComponent;

    async fn start() -> Result<(), String> {
        E::load(ExtensionAPI::new(Rc::new(host_api::GeneratedHost))).await
    }

    fn release(handler: &host::Callback) {
        callbacks::release(handler.id());
    }

    fn prepare_arguments(
        handler: &host::Callback,
        raw: String,
        emitter: &host::CallbackEmitter,
    ) -> Result<String, String> {
        let prepare = callbacks::find::<PrepareArguments>(handler)?;
        let raw = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        let emitter = contexts::GeneratedEmitter(emitter);
        prepare(raw, CallbackEmitter::new(&emitter)).map(|prepared| prepared.to_string())
    }

    async fn invoke_tool(
        handler: &host::Callback,
        tool_call_id: String,
        params: String,
        signal: Option<host::AbortSignal>,
        update: Option<host::ToolUpdate>,
        ctx: host::Context,
    ) -> Result<wire_models::AgentToolResult, String> {
        let execute = callbacks::find::<ToolExecute>(handler)?;
        let params = serde_json::from_str(&params).map_err(|error| error.to_string())?;
        let signal = signal.map(contexts::signal);
        let update = update.map(contexts::progress);
        execute(tool_call_id, params, signal, update, contexts::context(ctx)).await
    }

    async fn invoke_completions(
        handler: &host::Callback,
        prefix: String,
    ) -> Result<Option<Vec<wire_session::AutocompleteItem>>, String> {
        callbacks::find::<ArgumentCompletions>(handler)?(prefix).await
    }

    async fn invoke_shortcut(handler: &host::Callback, ctx: host::Context) -> Result<(), String> {
        callbacks::find::<ShortcutHandler>(handler)?(contexts::context(ctx)).await
    }

    async fn invoke_setup(
        handler: &host::Callback,
        manager: host::SessionManager,
    ) -> Result<(), String> {
        callbacks::take::<SetupSession>(handler)?(session::writer(manager)).await
    }

    fn invoke_listener(
        handler: &host::Callback,
        data: String,
        emitter: &host::CallbackEmitter,
    ) -> Result<(), String> {
        let listener = callbacks::find::<EventListener>(handler)?;
        let data = serde_json::from_str(&data).map_err(|error| error.to_string())?;
        let emitter = contexts::GeneratedEmitter(emitter);
        if let Some(rest) = listener(data, CallbackEmitter::new(&emitter))? {
            let tail = callbacks::Identity::new();
            host::announce_listener_tail(handler, &tail.handle);
            tail.keep(rest);
        }
        Ok(())
    }

    async fn invoke_listener_tail(tail: &host::Callback) -> Result<(), String> {
        let rest = callbacks::take::<callbacks::Tail>(tail)?;
        let outcome = rest.await;
        callbacks::release(tail.id());
        outcome
    }

    fn invoke_renderer(
        handler: &host::Callback,
        message: wire_models::CustomMessage,
        options: guest::MessageRenderOptions,
        theme: host::Theme,
        emitter: &host::CallbackEmitter,
    ) -> Result<Option<guest::Component>, String> {
        let renderer = callbacks::find::<MessageRenderer>(handler)?;
        let emitter = contexts::GeneratedEmitter(emitter);
        let component = renderer(
            message,
            options,
            contexts::theme(theme),
            CallbackEmitter::new(&emitter),
        )?;
        Ok(component.map(|component| guest::Component::new(ExportedComponent { component })))
    }

    fn invoke_compaction_complete(
        handler: &host::Callback,
        compaction: wire_session::CompactionResult,
        emitter: &host::CallbackEmitter,
    ) -> Result<(), String> {
        let complete = callbacks::find::<CompactionComplete>(handler)?;
        complete(
            compaction,
            CallbackEmitter::new(&contexts::GeneratedEmitter(emitter)),
        )
    }

    fn invoke_compaction_error(
        handler: &host::Callback,
        message: String,
        emitter: &host::CallbackEmitter,
    ) -> Result<(), String> {
        let failed = callbacks::find::<CompactionError>(handler)?;
        failed(
            message,
            CallbackEmitter::new(&contexts::GeneratedEmitter(emitter)),
        )
    }

    async fn invoke_command(
        handler: &host::Callback,
        args: String,
        ctx: host::CommandContext,
    ) -> Result<(), String> {
        let command = callbacks::find::<CommandHandler>(handler)?;
        command(args, contexts::command_context(ctx)).await
    }

    async fn invoke_with_session(
        handler: &host::Callback,
        ctx: host::ReplacedSessionContext,
    ) -> Result<(), String> {
        callbacks::take::<WithSession>(handler)?(contexts::replaced_context(ctx)).await
    }

    async fn invoke_agent_start(handler: &host::Callback, ctx: host::Context) -> guest::Decision {
        run(handler, &mut ExtensionEvent::AgentStart, ctx).await.0
    }

    async fn invoke_session_before_compact(
        handler: &host::Callback,
        event: wire::SessionBeforeCompactEventData,
        signal: host::AbortSignal,
        ctx: host::Context,
    ) -> guest::Decision {
        let signal = contexts::signal(signal);
        let event = crate::types::SessionBeforeCompactEvent {
            data: event,
            signal,
        };
        run(
            handler,
            &mut ExtensionEvent::Session(SessionEvent::BeforeCompact(event)),
            ctx,
        )
        .await
        .0
    }

    async fn invoke_session_before_tree(
        handler: &host::Callback,
        event: wire::SessionBeforeTreeEventData,
        signal: host::AbortSignal,
        ctx: host::Context,
    ) -> guest::Decision {
        let signal = contexts::signal(signal);
        let event = crate::types::SessionBeforeTreeEvent {
            data: event,
            signal,
        };
        run(
            handler,
            &mut ExtensionEvent::Session(SessionEvent::BeforeTree(event)),
            ctx,
        )
        .await
        .0
    }

    async fn invoke_tool_call(
        handler: &host::Callback,
        event: wire::ToolCallEvent,
        ctx: host::Context,
    ) -> guest::ToolCallOutcome {
        let mut event = ExtensionEvent::ToolCall(event);
        let (decision, _) = run(handler, &mut event, ctx).await;
        let event = match event {
            ExtensionEvent::ToolCall(event) => Some(event),
            _ => None,
        };
        guest::ToolCallOutcome { event, decision }
    }

    async fn invoke_user_bash(
        handler: &host::Callback,
        event: wire::UserBashEvent,
        ctx: host::Context,
    ) -> (guest::Decision, Option<guest::BashOperations>) {
        let (decision, operations) = run(handler, &mut ExtensionEvent::UserBash(event), ctx).await;
        let operations = operations.map(|operations| {
            guest::BashOperations::new(ExportedBashOperations {
                _operations: operations,
            })
        });
        (decision, operations)
    }

    event_exports! {
        invoke_resources_discover(wire::ResourcesDiscoverEvent) => ExtensionEvent::ResourcesDiscover;
        invoke_session_start(wire::SessionStartEvent) => |e| ExtensionEvent::Session(SessionEvent::Start(e));
        invoke_session_before_switch(wire::SessionBeforeSwitchEvent) => |e| ExtensionEvent::Session(SessionEvent::BeforeSwitch(e));
        invoke_session_before_fork(wire::SessionBeforeForkEvent) => |e| ExtensionEvent::Session(SessionEvent::BeforeFork(e));
        invoke_session_compact(wire::SessionCompactEvent) => |e| ExtensionEvent::Session(SessionEvent::Compact(e));
        invoke_session_shutdown(wire::SessionShutdownEvent) => |e| ExtensionEvent::Session(SessionEvent::Shutdown(e));
        invoke_session_tree(wire::SessionTreeEvent) => |e| ExtensionEvent::Session(SessionEvent::Tree(e));
        invoke_context(wire::ContextEvent) => ExtensionEvent::Context;
        invoke_before_provider_request(wire::BeforeProviderRequestEvent) => ExtensionEvent::BeforeProviderRequest;
        invoke_after_provider_response(wire::AfterProviderResponseEvent) => ExtensionEvent::AfterProviderResponse;
        invoke_before_agent_start(wire::BeforeAgentStartEvent) => |e| ExtensionEvent::BeforeAgentStart(Box::new(e));
        invoke_agent_end(wire::AgentEndEvent) => ExtensionEvent::AgentEnd;
        invoke_turn_start(wire::TurnStartEvent) => ExtensionEvent::TurnStart;
        invoke_turn_end(wire::TurnEndEvent) => ExtensionEvent::TurnEnd;
        invoke_message_start(wire::MessageStartEvent) => ExtensionEvent::MessageStart;
        invoke_message_update(wire::MessageUpdateEvent) => |e| ExtensionEvent::MessageUpdate(Box::new(e));
        invoke_message_end(wire::MessageEndEvent) => ExtensionEvent::MessageEnd;
        invoke_tool_execution_start(wire::ToolExecutionStartEvent) => ExtensionEvent::ToolExecutionStart;
        invoke_tool_execution_update(wire::ToolExecutionUpdateEvent) => ExtensionEvent::ToolExecutionUpdate;
        invoke_tool_execution_end(wire::ToolExecutionEndEvent) => ExtensionEvent::ToolExecutionEnd;
        invoke_model_select(wire::ModelSelectEvent) => |e| ExtensionEvent::ModelSelect(Box::new(e));
        invoke_thinking_level_select(wire::ThinkingLevelSelectEvent) => ExtensionEvent::ThinkingLevelSelect;
        invoke_input(wire::InputEvent) => ExtensionEvent::Input;
        invoke_tool_result(wire::ToolResultEvent) => ExtensionEvent::ToolResult;
        invoke_custom(wire::CustomEvent) => ExtensionEvent::Custom;
    }
}
