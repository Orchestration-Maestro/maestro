//! The exports of the extension world, served over any set of host imports.

use std::rc::Rc;

use super::callbacks;
use super::contexts;
use super::host_api::Host;
use super::imports::Imports;
use crate::bindings::exports::maestro::extension::guest::{
    Decision, InputOutcome, SessionBeforeCompactOutcome,
};
use crate::bindings::maestro::extension::events as wire;
use crate::loader::Extension;
use crate::types::{
    CommandHandler, ExtensionAPI, ExtensionEvent, ExtensionEventResult, ExtensionHandler,
    ExtensionResult, SessionBeforeCompactEvent, SessionEvent, WithSession,
};

/// Serves the exports an extension component provides on top of its host.
pub(crate) struct Exports<I: Imports> {
    /// The host the extension runs against.
    imports: I,
}

/// The wire form of a result a handler returned.
fn wire_result(result: ExtensionEventResult) -> wire::ExtensionEventResult {
    match result {
        ExtensionEventResult::SessionBeforeCompact(result) => {
            wire::ExtensionEventResult::SessionBeforeCompact(result)
        }
        ExtensionEventResult::Input(result) => wire::ExtensionEventResult::Input(result),
    }
}

impl<I: Imports> Exports<I> {
    /// Serves exports over `imports`.
    pub(crate) fn new(imports: I) -> Self {
        Self { imports }
    }

    /// Runs the extension factory of `E`; resolves when it has completed.
    pub(crate) async fn start<E: Extension>(&self) -> ExtensionResult<()> {
        E::load(ExtensionAPI::new(Rc::new(Host(self.imports.clone())))).await
    }

    /// Runs the command handler registered under `handler`.
    pub(crate) async fn invoke_command(
        &self,
        handler: u32,
        args: String,
        ctx: I::CommandContext,
    ) -> ExtensionResult<()> {
        let command = callbacks::find::<CommandHandler>(handler)?;
        command(args, contexts::command_context(&self.imports, ctx)).await
    }

    /// Runs the continuation pending under `handler`.
    pub(crate) async fn invoke_with_session(
        &self,
        handler: u32,
        ctx: I::ReplacedContext,
    ) -> ExtensionResult<()> {
        callbacks::take::<WithSession>(handler)?(contexts::replaced_context(&self.imports, ctx))
            .await
    }

    /// Runs the handler registered under `handler` against an event the host delivered.
    async fn run(&self, handler: u32, event: &mut ExtensionEvent, ctx: I::Context) -> Decision {
        let handler = callbacks::find::<ExtensionHandler>(handler)?;
        let result = handler(event, contexts::context(&self.imports, ctx)).await?;
        Ok(result.map(wire_result))
    }

    /// Delivers a compaction that is about to happen; returns the event as the handler left it
    /// with the handler's decision.
    pub(crate) async fn invoke_session_before_compact(
        &self,
        handler: u32,
        event: wire::SessionBeforeCompactEventData,
        signal: I::Signal,
        ctx: I::Context,
    ) -> SessionBeforeCompactOutcome {
        let event = SessionBeforeCompactEvent {
            data: event,
            signal: contexts::signal(&self.imports, signal),
        };
        let mut event = ExtensionEvent::Session(SessionEvent::BeforeCompact(event));
        let decision = self.run(handler, &mut event, ctx).await;
        SessionBeforeCompactOutcome {
            event: match event {
                ExtensionEvent::Session(SessionEvent::BeforeCompact(edited)) => Some(edited.data),
                ExtensionEvent::Input(_) => None,
            },
            decision,
        }
    }

    /// Delivers a user input; returns the event as the handler left it with the handler's
    /// decision.
    pub(crate) async fn invoke_input(
        &self,
        handler: u32,
        event: wire::InputEvent,
        ctx: I::Context,
    ) -> InputOutcome {
        let mut event = ExtensionEvent::Input(event);
        let decision = self.run(handler, &mut event, ctx).await;
        InputOutcome {
            event: match event {
                ExtensionEvent::Input(edited) => Some(edited),
                ExtensionEvent::Session(_) => None,
            },
            decision,
        }
    }
}
