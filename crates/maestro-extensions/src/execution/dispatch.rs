//! Typed dispatch over live extension registrations.
use super::ExtensionRunner;
use crate::{ExtensionExecution, types::read};

use crate::{
    EventError, ExtensionContext, ExtensionHandler,
    types::{
        ExtensionHandle,
        events::{
            AfterProviderResponseEvent, AgentEndEvent, AgentStartEvent, MessageStartEvent,
            MessageUpdateEvent, ModelSelectEvent, SessionBeforeCompactEvent,
            SessionBeforeCompactResult, SessionBeforeForkEvent, SessionBeforeForkResult,
            SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionBeforeTreeEvent,
            SessionBeforeTreeResult, SessionCompactEvent, SessionShutdownEvent, SessionStartEvent,
            SessionTreeEvent, ThinkingLevelSelectEvent, ToolExecutionEndEvent,
            ToolExecutionStartEvent, ToolExecutionUpdateEvent, TurnEndEvent, TurnStartEvent,
        },
    },
};
use maestro_session::Presence;

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Whether any current extension has callbacks for this exact event name.
    #[must_use]
    pub fn has_handlers(&self, event_type: &str) -> bool {
        read(&self.extensions).iter().any(|owner| {
            read(owner)
                .handlers
                .get(event_type)
                .is_some_and(|handlers| !handlers.is_empty())
        })
    }
}

/// Events admitted by the notification and session-before dispatcher.
pub trait RunnerEmitEvent: sealed::Sealed + 'static {
    /// The result of this event's callback.
    type Result: 'static;
    /// Exact registered event name.
    const EVENT_TYPE: &'static str;
    /// Whether this result ends dispatch immediately.
    fn cancels(result: &Self::Result) -> bool;
}
/// Restricts generic dispatch to its supported event forms.
mod sealed {
    /// Marker for supported event records.
    pub trait Sealed {}
}
/// Implements result-free notification dispatch.
macro_rules! notify {
    ($($ty:ty => $name:literal),* $(,)?) => {$(
        impl sealed::Sealed for $ty {}
        impl RunnerEmitEvent for $ty {
            type Result = ();
            const EVENT_TYPE: &'static str = $name;
            fn cancels(_: &()) -> bool { false }
        }
    )*};
}
notify! {
    SessionStartEvent => "session_start", SessionCompactEvent => "session_compact",
    SessionShutdownEvent => "session_shutdown", SessionTreeEvent => "session_tree",
    AfterProviderResponseEvent => "after_provider_response", AgentStartEvent => "agent_start",
    TurnStartEvent => "turn_start", ToolExecutionStartEvent => "tool_execution_start",
    ToolExecutionUpdateEvent => "tool_execution_update", ToolExecutionEndEvent => "tool_execution_end",
    ModelSelectEvent => "model_select", ThinkingLevelSelectEvent => "thinking_level_select",
}
/// Implements notifications carrying caller-owned message types.
macro_rules! message_notify {
    ($($ty:ident => $name:literal),* $(,)?) => {$(
        impl<M: 'static> sealed::Sealed for $ty<M> {}
        impl<M: 'static> RunnerEmitEvent for $ty<M> {
            type Result = ();
            const EVENT_TYPE: &'static str = $name;
            fn cancels(_: &()) -> bool { false }
        }
    )*};
}
message_notify! {
    AgentEndEvent => "agent_end", TurnEndEvent => "turn_end",
    MessageStartEvent => "message_start", MessageUpdateEvent => "message_update",
}
/// Implements independently typed session veto results.
macro_rules! before {
    ($($ty:ty => $result:ty => $name:literal),* $(,)?) => {$(
        impl sealed::Sealed for $ty {}
        impl RunnerEmitEvent for $ty {
            type Result = $result;
            const EVENT_TYPE: &'static str = $name;
            fn cancels(result: &Self::Result) -> bool {
                matches!(result.cancel, Presence::Present(true))
            }
        }
    )*};
}
before! {
    SessionBeforeSwitchEvent => SessionBeforeSwitchResult => "session_before_switch",
    SessionBeforeForkEvent => SessionBeforeForkResult => "session_before_fork",
    SessionBeforeCompactEvent => SessionBeforeCompactResult => "session_before_compact",
    SessionBeforeTreeEvent => SessionBeforeTreeResult => "session_before_tree",
}

/// Monotone traversal retaining the active owner across callbacks and suspension.
pub(super) struct HandlerCursor<E: ExtensionExecution> {
    /// Next extension slot in live load order.
    extension: usize,
    /// Next registration within the retained extension.
    handler: usize,
    /// The extension whose handlers are currently being traversed.
    owner: Option<ExtensionHandle<E>>,
}
impl<E: ExtensionExecution> Default for HandlerCursor<E> {
    fn default() -> Self {
        Self {
            extension: 0,
            handler: 0,
            owner: None,
        }
    }
}
impl<E: ExtensionExecution> HandlerCursor<E> {
    /// Selects one typed callback, releasing registration guards before returning.
    pub(super) fn next<V: 'static, R: 'static>(
        &mut self,
        runner: &ExtensionRunner<E>,
        event_type: &'static str,
    ) -> Option<Invocation<V, R, E>> {
        loop {
            if self.owner.is_none() {
                self.owner = read(&runner.extensions).get(self.extension).cloned();
            }
            let owner = self.owner.as_ref()?;
            let next = {
                let registrations = read(owner);
                registrations
                    .handlers
                    .get(event_type)
                    .and_then(|handlers| handlers.get(self.handler).map(E::recover_handler::<V, R>))
            };
            let Some(handler) = next else {
                self.owner = None;
                self.extension += 1;
                self.handler = 0;
                continue;
            };
            self.handler += 1;
            let Some(handler) = handler else {
                continue;
            };
            return Some(Invocation {
                owner: owner.clone(),
                handler,
                event_type,
            });
        }
    }
}
/// A selected callback and its live attribution owner.
pub(super) struct Invocation<V, R, E: ExtensionExecution> {
    /// Retained registration owner, read again after the callback completes.
    pub(super) owner: ExtensionHandle<E>,
    /// Typed callback retained independently of the registration guard.
    pub(super) handler: ExtensionHandler<V, R, E>,
    /// Literal event name for error attribution.
    event_type: &'static str,
}
impl<V, R, E: ExtensionExecution> Invocation<V, R, E> {
    /// Calls a catching event handler and reports failures outside all guards.
    pub(super) async fn call(
        &self,
        runner: &ExtensionRunner<E>,
        event: &mut V,
        context: ExtensionContext,
    ) -> Result<Option<R>, EventError> {
        match (self.handler)(event, context).await {
            Ok(result) => Ok(result),
            Err(error) => {
                let path = read(&self.owner).path.clone();
                runner.emit_error(&super::errors::event_error(path, self.event_type, &error))?;
                Ok(None)
            }
        }
    }
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Awaits live ordered callbacks, retaining session-before results and vetoes.
    /// # Errors
    /// Returns an error-listener failure without invoking later callbacks.
    pub async fn emit<V: RunnerEmitEvent>(
        &self,
        event: &mut V,
    ) -> Result<Option<V::Result>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut result = None;
        while let Some(invocation) = cursor.next::<V, V::Result>(self, V::EVENT_TYPE) {
            let Some(next) = invocation.call(self, event, context.clone()).await? else {
                continue;
            };
            if V::cancels(&next) {
                return Ok(Some(next));
            }
            if matches!(
                V::EVENT_TYPE,
                "session_before_switch"
                    | "session_before_fork"
                    | "session_before_compact"
                    | "session_before_tree"
            ) {
                result = Some(next);
            }
        }
        Ok(result)
    }
}
/// Emits shutdown when handlers exist, without tearing down registrations.
/// # Errors
/// Returns an error-listener failure.
pub async fn emit_session_shutdown_event<E: ExtensionExecution>(
    runner: &ExtensionRunner<E>,
    event: &mut SessionShutdownEvent,
) -> Result<bool, EventError> {
    if !runner.has_handlers("session_shutdown") {
        return Ok(false);
    }
    runner.emit(event).await?;
    Ok(true)
}
