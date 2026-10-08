//! Component adapter: forwards registrations to the generated imports and
//! serves the generated exports from a private closure table.
//!
//! Closures stay in this guest, keyed by the identity of the host-owned callback
//! resource that announces them. The table is never borrowed while author code runs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::Rc;

use serde_json::Value;

use crate::api::{
    Extension, ExtensionAPI, ExtensionFuture, ExtensionHost, ExtensionResult, InputHandler,
};
use crate::bindings::exports::maestro::extension::guest::{self, Guest, GuestComponent};
use crate::bindings::maestro::extension::{host, types};
use crate::component::{ComponentView, MessageRenderer};
use crate::context::{
    AbortSignal, CommandContextPort, CommandHandler, ContextPort, ExtensionCommandContext,
    ExtensionContext, NewSessionCommandOptions, ReplacedSessionContext, ReplacedSessionContextPort,
    SignalPort, WithSession,
};
use crate::dispatch::{ToolInvocation, prepare_arguments, run_input, run_tool};
use crate::tool::{
    AgentToolResult, AgentToolUpdateCallback, PrepareArguments, ToolDefinition, ToolExecute,
};

/// A closure the table keeps for one host identity.
enum Kind {
    /// Input handler.
    Input(InputHandler),
    /// Argument preparation of a tool.
    Prepare(PrepareArguments),
    /// Body of a tool.
    Execute(ToolExecute),
    /// Command handler.
    Command(CommandHandler),
    /// Message renderer.
    Renderer(MessageRenderer),
    /// Continuation of a new session; taken when it runs.
    WithSession(Option<WithSession>),
}

/// One table entry: the closure and, for registered callbacks, its host identity.
struct Entry {
    /// The closure.
    kind: Kind,
    /// Keeps a registered callback's host identity alive until the entry is released;
    /// operation-scoped entries hold theirs in `Scoped`.
    _handle: Option<types::Callback>,
}

thread_local! {
    static REGISTRY: RefCell<HashMap<u32, Entry>> = RefCell::new(HashMap::new());
}

/// A host identity that is not in the table yet.
struct Fresh {
    /// The host-owned resource.
    handle: types::Callback,
    /// Key of the identity.
    id: u32,
}

/// Asks the host for a fresh identity.
fn fresh() -> Fresh {
    let handle = types::Callback::new();
    let id = handle.id();
    Fresh { handle, id }
}

/// Keeps the closure of a registered callback.
fn keep(fresh: Fresh, kind: Kind) {
    let Fresh { handle, id } = fresh;
    store(
        id,
        Entry {
            kind,
            _handle: Some(handle),
        },
    );
}

/// Inserts an entry without running author code under the table borrow.
fn store(id: u32, entry: Entry) {
    let replaced = REGISTRY.with_borrow_mut(|registry| registry.insert(id, entry));
    drop(replaced);
}

/// Removes an entry and drops its closure after the table borrow ended.
fn release_entry(id: u32) {
    let removed = REGISTRY.with_borrow_mut(|registry| registry.remove(&id));
    drop(removed);
}

/// Finds the closure registered for an identity.
fn find<T>(handler: &types::Callback, pick: impl FnOnce(&Kind) -> Option<T>) -> ExtensionResult<T> {
    let id = handler.id();
    REGISTRY
        .with_borrow(|registry| registry.get(&id).and_then(|entry| pick(&entry.kind)))
        .ok_or_else(|| format!("no callback registered for identity {id}"))
}

/// An operation-scoped callback: the closure lives until the operation completes.
struct Scoped {
    /// The host-owned resource.
    handle: types::Callback,
    /// Key of the identity.
    id: u32,
}

impl Scoped {
    /// Registers a closure that lives only for one operation.
    fn new(kind: Kind) -> Self {
        let Fresh { handle, id } = fresh();
        store(
            id,
            Entry {
                kind,
                _handle: None,
            },
        );
        Self { handle, id }
    }
}

impl Drop for Scoped {
    fn drop(&mut self) {
        release_entry(self.id);
    }
}

/// Entry type of an extension component: serves the generated exports for `E`.
pub struct Glue<E>(PhantomData<E>);

impl<E: Extension> Guest for Glue<E> {
    type Component = ComponentExport;

    async fn start() -> Result<(), String> {
        E::load(ExtensionAPI::new(Rc::new(GeneratedHost))).await
    }

    fn release(handler: &types::Callback) {
        release_entry(handler.id());
    }

    async fn invoke_input(
        handler: &types::Callback,
        event: types::InputEvent,
        ctx: types::Context,
    ) -> types::InputOutcome {
        let found = find(handler, |kind| match kind {
            Kind::Input(handler) => Some(Rc::clone(handler)),
            _ => None,
        });
        match found {
            Ok(handler) => run_input(&handler, event, context(ctx)).await,
            Err(message) => types::InputOutcome {
                event,
                decision: Err(message),
            },
        }
    }

    fn prepare_arguments(handler: &types::Callback, raw: String) -> Result<String, String> {
        let prepare = find(handler, |kind| match kind {
            Kind::Prepare(prepare) => Some(Rc::clone(prepare)),
            _ => None,
        })?;
        prepare_arguments(&prepare, &raw)
    }

    async fn invoke_tool(
        handler: &types::Callback,
        call_id: String,
        params: String,
        signal: Option<types::AbortSignal>,
        update: Option<types::ToolUpdate>,
        ctx: types::Context,
    ) -> Result<types::ToolResult, String> {
        let execute = find(handler, |kind| match kind {
            Kind::Execute(execute) => Some(Rc::clone(execute)),
            _ => None,
        })?;
        let call = ToolInvocation {
            call_id,
            params,
            signal: signal.map(|signal| AbortSignal::new(Rc::new(GeneratedSignal(signal)))),
            update: update.map(progress),
        };
        run_tool(&execute, call, context(ctx)).await
    }

    async fn invoke_command(
        handler: &types::Callback,
        args: String,
        ctx: types::CommandContext,
    ) -> Result<(), String> {
        let command = find(handler, |kind| match kind {
            Kind::Command(command) => Some(Rc::clone(command)),
            _ => None,
        })?;
        command(args, command_context(ctx)).await
    }

    async fn invoke_with_session(
        handler: &types::Callback,
        ctx: types::ReplacedSessionContext,
    ) -> Result<(), String> {
        let id = handler.id();
        let taken = REGISTRY.with_borrow_mut(|registry| {
            match registry.get_mut(&id).map(|entry| &mut entry.kind) {
                Some(Kind::WithSession(slot)) => slot.take(),
                _ => None,
            }
        });
        let with_session =
            taken.ok_or_else(|| format!("no pending continuation for identity {id}"))?;
        with_session(ReplacedSessionContext::new(Rc::new(GeneratedReplaced(ctx)))).await
    }

    fn invoke_renderer(
        handler: &types::Callback,
        message: types::CustomMessage,
        expanded: bool,
    ) -> Result<Option<guest::Component>, String> {
        let renderer = find(handler, |kind| match kind {
            Kind::Renderer(renderer) => Some(Rc::clone(renderer)),
            _ => None,
        })?;
        Ok(renderer(message, expanded)?
            .map(|component| guest::Component::new(ComponentExport(component.into_view()))))
    }
}

/// Exported component resource wrapping an author's view.
pub struct ComponentExport(Box<dyn ComponentView>);

impl GuestComponent for ComponentExport {
    fn render(&self, width: u32) -> Vec<String> {
        self.0.render(width)
    }

    fn invalidate(&self) {
        self.0.invalidate();
    }
}

/// Wraps a generated context.
fn context(ctx: types::Context) -> ExtensionContext {
    ExtensionContext::new(Rc::new(GeneratedContext(ctx)))
}

/// Wraps a generated command context.
fn command_context(ctx: types::CommandContext) -> ExtensionCommandContext {
    ExtensionCommandContext::new(Rc::new(GeneratedCommand(ctx)))
}

/// Wraps a generated progress channel as a callback.
fn progress(update: types::ToolUpdate) -> AgentToolUpdateCallback {
    Rc::new(move |partial: AgentToolResult| update.send(&partial.into()))
}

/// Wraps a generated signal.
fn signal_of(signal: Option<types::AbortSignal>) -> Option<AbortSignal> {
    signal.map(|signal| AbortSignal::new(Rc::new(GeneratedSignal(signal))))
}

/// Generated signal behind the facade.
struct GeneratedSignal(types::AbortSignal);

impl SignalPort for GeneratedSignal {
    fn aborted(&self) -> bool {
        self.0.aborted()
    }
}

/// Generated context behind the facade.
struct GeneratedContext(types::Context);

impl ContextPort for GeneratedContext {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.0.signal().map(signal_of)
    }
}

/// Generated command context behind the facade.
struct GeneratedCommand(types::CommandContext);

impl ContextPort for GeneratedCommand {
    fn cwd(&self) -> ExtensionResult<String> {
        self.0.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.0.signal().map(signal_of)
    }
}

impl CommandContextPort for GeneratedCommand {
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, types::SessionChangeResult> {
        Box::pin(async move {
            let NewSessionCommandOptions {
                parent_session,
                with_session,
            } = options.unwrap_or_default();
            let scoped =
                with_session.map(|with_session| Scoped::new(Kind::WithSession(Some(with_session))));
            let announce = scoped.as_ref().map(|scoped| &scoped.handle);
            self.0
                .new_session(types::NewSessionOptions { parent_session }, announce)
                .await
        })
    }
}

/// Generated replacement context behind the facade.
struct GeneratedReplaced(types::ReplacedSessionContext);

impl ReplacedSessionContextPort for GeneratedReplaced {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(GeneratedCommand(self.0.command()))
    }

    fn send_user_message(&self, text: &str) -> ExtensionFuture<'_, ()> {
        let text = text.to_owned();
        Box::pin(async move { self.0.send_user_message(text).await })
    }
}

/// Forwards registrations to the generated imports.
struct GeneratedHost;

impl ExtensionHost for GeneratedHost {
    fn on_input(&self, handler: InputHandler) -> ExtensionResult<()> {
        let callback = fresh();
        host::on_input(&callback.handle)?;
        keep(callback, Kind::Input(handler));
        Ok(())
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        let ToolDefinition {
            metadata,
            prepare_arguments,
            execute,
        } = tool;
        let prepare = prepare_arguments.map(|prepare| (fresh(), prepare));
        let execute_callback = fresh();
        host::register_tool(
            &metadata,
            prepare.as_ref().map(|(callback, _)| &callback.handle),
            &execute_callback.handle,
        )?;
        keep(execute_callback, Kind::Execute(execute));
        if let Some((callback, prepare)) = prepare {
            keep(callback, Kind::Prepare(prepare));
        }
        Ok(())
    }

    fn register_command(&self, name: &str, handler: CommandHandler) -> ExtensionResult<()> {
        let callback = fresh();
        host::register_command(name, &callback.handle)?;
        keep(callback, Kind::Command(handler));
        Ok(())
    }

    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: MessageRenderer,
    ) -> ExtensionResult<()> {
        let callback = fresh();
        host::register_message_renderer(custom_type, &callback.handle)?;
        keep(callback, Kind::Renderer(renderer));
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<&Value>) -> ExtensionResult<()> {
        host::append_entry(custom_type, data.map(Value::to_string).as_deref())
    }
}
