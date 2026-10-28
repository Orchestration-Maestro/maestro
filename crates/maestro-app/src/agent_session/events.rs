//! The agent-event dispatch: one ordered queue that tells the extensions, then the session
//! listeners, persists finished messages and decides on retry and compaction.
use super::held::heard_while_held;
use super::state::{Gate, SharedAgentEvent, thrown};
use super::{AgentSession, AgentSessionEvent};
use crate::utils::lock::{read, write};
use futures_util::FutureExt;
use maestro_agent::{AgentEvent, AgentListener, AgentMessage};
use maestro_extensions::{
    AgentEndEvent, AgentStartEvent, ExtensionExecution, ExtensionRunner, MessageEndEvent,
    MessageStartEvent, MessageUpdateEvent, NativeAgentMessage, ToolExecutionEndEvent,
    ToolExecutionStartEvent, ToolExecutionUpdateEvent, TurnEndEvent, TurnStartEvent,
};
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, Local, StopReason, UserContent, UserMessage,
    timestamp_now,
};
use maestro_session::messages::CustomAgentMessages;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// The session's one agent listener: it opens the retry observation of a failed run at once,
/// then queues the event for ordered processing.
pub(super) fn agent_listener(
    session: &Weak<AgentSession<Local>>,
) -> AgentListener<CustomAgentMessages, Local> {
    let session = Weak::clone(session);
    Rc::new(move |event, _signal| {
        if let Some(session) = session.upgrade() {
            session.create_retry_gate_for_agent_end(&read(&event));
            session.enqueue_agent_event(event);
        }
        Box::pin(std::future::ready(Ok(())))
    })
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Tells the session listeners about `event`, the one place they are told of any. With
    /// `holdUncheckedText` set, an event that carries assistant text, thinking or partial tool
    /// output reaches them without it, or not at all, until the message or tool result is
    /// complete.
    pub(super) fn emit_agent_event(&self, event: SharedAgentEvent) {
        let heard = if self.settings_manager.borrow().get_hold_unchecked_text() {
            heard_while_held(&event)
        } else {
            Some(event)
        };
        if let Some(heard) = heard {
            self.emit(&AgentSessionEvent::Agent(heard));
        }
    }
}

impl AgentSession<Local> {
    /// Appends `event` to the queue and starts processing it when nothing is being
    /// processed; a failing event is dropped and the rest still run.
    fn enqueue_agent_event(self: &Rc<Self>, event: SharedAgentEvent) {
        let mut state = self.state.borrow_mut();
        state.agent_events.push_back(event);
        if state.agent_event_drain.is_some() {
            return;
        }
        let (open, closed) = tokio::sync::oneshot::channel::<()>();
        state.agent_event_drain = Some(closed.shared());
        drop(state);
        let session = Rc::clone(self);
        drop(self.start_operation(async move {
            while let Some(event) = session.next_agent_event() {
                drop(session.process_agent_event(&event).await);
            }
            drop(open);
        }));
    }

    /// The next queued event; when none is left, the queue is marked processed.
    fn next_agent_event(&self) -> Option<SharedAgentEvent> {
        let mut state = self.state.borrow_mut();
        let next = state.agent_events.pop_front();
        if next.is_none() {
            state.agent_event_drain = None;
        }
        next
    }

    /// Released once every queued agent event is processed; absent when none is queued.
    pub(super) fn agent_event_queue(&self) -> Option<Gate> {
        self.state.borrow().agent_event_drain.clone()
    }

    /// Opens the retry observation when a run ends on a response a retry recovers, before
    /// the queued processing of that end.
    fn create_retry_gate_for_agent_end(&self, event: &AgentEvent<CustomAgentMessages>) {
        let AgentEvent::AgentEnd { messages } = event else {
            return;
        };
        if self.state.borrow().retry_gate.is_some()
            || !self.settings_manager.borrow().get_retry_settings().enabled
        {
            return;
        }
        let last = super::state::last_assistant_message(messages);
        if last.is_some_and(|last| self.is_retryable_error(&read(&last))) {
            self.create_retry_gate();
        }
    }

    /// Processes one agent event: removes a delivered queued text, tells the extensions,
    /// then the listeners, persists a finished message and, at the run's end, retries or
    /// compacts.
    async fn process_agent_event(
        self: &Rc<Self>,
        event: &SharedAgentEvent,
    ) -> Result<(), DiagnosticErrorInfo> {
        let started_user = match &*read(event) {
            AgentEvent::MessageStart {
                message: AgentMessage::User(user),
            } => Some(Arc::clone(user)),
            _ => None,
        };
        if let Some(user) = started_user {
            self.clear_overflow_recovery_attempt();
            let text = user_message_text(&read(&user));
            self.remove_delivered_text(&text);
        }
        self.emit_extension_event(event).await?;
        self.emit_agent_event(Arc::clone(event));
        let (finished, run_ended) = match &*read(event) {
            AgentEvent::MessageEnd { message } => (Some(message.clone()), false),
            AgentEvent::AgentEnd { .. } => (None, true),
            _ => (None, false),
        };
        if let Some(message) = finished {
            self.persist_finished_message(&message)?;
        }
        let last = run_ended
            .then(|| self.state.borrow_mut().last_assistant_message.take())
            .flatten();
        if let Some(last) = last {
            let last = read(&last).clone();
            self.retry_or_compact(&last).await?;
        }
        Ok(())
    }

    /// Retries a recoverable failure; otherwise releases the retry observers and checks
    /// compaction.
    async fn retry_or_compact(
        self: &Rc<Self>,
        message: &AssistantMessage,
    ) -> Result<(), DiagnosticErrorInfo> {
        if self.is_retryable_error(message) && self.handle_retryable_error(message).await? {
            return Ok(());
        }
        self.resolve_retry();
        self.check_compaction(message, true).await
    }

    /// Removes `text` from the steering mirror, else from the follow-up mirror, and
    /// publishes both when one changed.
    fn remove_delivered_text(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let removed = {
            let mut state = self.state.borrow_mut();
            let state = &mut *state;
            [&mut state.steering_messages, &mut state.follow_up_messages]
                .into_iter()
                .any(|mirror| {
                    let found = mirror.iter().position(|queued| queued == text);
                    found.map(|index| mirror.remove(index)).is_some()
                })
        };
        if removed {
            self.emit_queue_update();
        }
    }

    /// Stores a finished message: a custom message as a custom entry, a user, assistant or
    /// tool-result message as a message entry. A finished response becomes the one checked
    /// at the run's end, and a successful one ends the retrying.
    fn persist_finished_message(
        &self,
        message: &NativeAgentMessage,
    ) -> Result<(), DiagnosticErrorInfo> {
        match message {
            AgentMessage::Custom(custom) => {
                if let CustomAgentMessages::Custom(custom) = &*read(custom) {
                    self.session_manager
                        .borrow_mut()
                        .append_custom_message_entry(
                            &custom.custom_type,
                            custom.content.clone(),
                            custom.display,
                            custom.details.clone(),
                        )
                        .map_err(thrown)?;
                }
            }
            AgentMessage::User(_) | AgentMessage::Assistant(_) | AgentMessage::ToolResult(_) => {
                let record = serde_json::to_value(message).map_err(thrown)?;
                self.session_manager
                    .borrow_mut()
                    .append_message(record)
                    .map_err(thrown)?;
            }
        }
        if let AgentMessage::Assistant(response) = message {
            self.finish_response(response);
        }
        Ok(())
    }

    /// Records `response` for the run's end; a successful one clears the overflow recovery
    /// and announces the end of a retry.
    fn finish_response(&self, response: &maestro_models::SharedAssistantMessage) {
        let failed = read(response).stop_reason == StopReason::Error;
        if !failed {
            self.clear_overflow_recovery_attempt();
        }
        let ended_retry = {
            let mut state = self.state.borrow_mut();
            state.last_assistant_message = Some(Arc::clone(response));
            let attempt = state.retry_attempt;
            (!failed && attempt > 0).then_some(attempt)
        };
        if let Some(attempt) = ended_retry {
            self.emit(&AgentSessionEvent::AutoRetryEnd {
                success: true,
                attempt,
                final_error: None,
            });
            self.state.borrow_mut().retry_attempt = 0;
        }
    }

    /// Tells the extensions about `event` in their own event form; an extension that
    /// rewrites a finished message replaces it where every later reader finds it.
    async fn emit_extension_event(
        &self,
        event: &SharedAgentEvent,
    ) -> Result<(), DiagnosticErrorInfo> {
        let runner = self.extension_runner();
        let copied = { read(event).clone() };
        match copied {
            AgentEvent::MessageEnd { message } => {
                let mut end = MessageEndEvent {
                    message: message.clone(),
                };
                let replacement = runner.emit_message_end(&mut end).await.map_err(thrown)?;
                if let Some(replacement) = replacement {
                    replace_message_in_place(&message, &replacement);
                }
                Ok(())
            }
            AgentEvent::AgentStart => {
                self.state.borrow_mut().turn_index = 0;
                runner
                    .emit(&mut AgentStartEvent {})
                    .await
                    .map(drop)
                    .map_err(thrown)
            }
            AgentEvent::TurnStart => {
                let turn_index = f64::from(self.state.borrow().turn_index);
                let mut turn = TurnStartEvent {
                    turn_index,
                    timestamp: timestamp_now(),
                };
                runner.emit(&mut turn).await.map(drop).map_err(thrown)
            }
            AgentEvent::TurnEnd {
                message,
                tool_results,
            } => {
                let mut turn = TurnEndEvent {
                    turn_index: f64::from(self.state.borrow().turn_index),
                    message: NativeAgentMessage::Assistant(message),
                    tool_results,
                };
                runner.emit(&mut turn).await.map_err(thrown)?;
                let mut state = self.state.borrow_mut();
                state.turn_index = state.turn_index.saturating_add(1);
                Ok(())
            }
            other => emit_untracked_event(&runner, other).await,
        }
    }
}

/// Tells the extensions about an agent event whose extension form needs no session state.
async fn emit_untracked_event(
    runner: &ExtensionRunner<Local>,
    event: AgentEvent<CustomAgentMessages>,
) -> Result<(), DiagnosticErrorInfo> {
    let emitted = match event {
        AgentEvent::AgentEnd { messages } => runner.emit(&mut AgentEndEvent { messages }).await,
        AgentEvent::MessageStart { message } => {
            runner.emit(&mut MessageStartEvent { message }).await
        }
        AgentEvent::MessageUpdate {
            message,
            assistant_message_event,
        } => {
            let mut update = MessageUpdateEvent {
                message: NativeAgentMessage::Assistant(message),
                assistant_message_event,
            };
            runner.emit(&mut update).await
        }
        AgentEvent::ToolExecutionStart { tool_call } => {
            let mut start = ToolExecutionStartEvent {
                tool_call_id: tool_call.id,
                tool_name: tool_call.name,
                args: tool_call.arguments,
            };
            runner.emit(&mut start).await
        }
        AgentEvent::ToolExecutionUpdate {
            tool_call,
            partial_result,
        } => {
            let mut update = ToolExecutionUpdateEvent {
                tool_call_id: tool_call.id,
                tool_name: tool_call.name,
                args: tool_call.arguments,
                partial_result: serde_json::to_value(partial_result).map_err(thrown)?,
            };
            runner.emit(&mut update).await
        }
        AgentEvent::ToolExecutionEnd {
            tool_call,
            result,
            is_error,
        } => {
            let mut end = ToolExecutionEndEvent {
                tool_call_id: tool_call.id,
                tool_name: tool_call.name,
                result: serde_json::to_value(result).map_err(thrown)?,
                is_error,
            };
            runner.emit(&mut end).await
        }
        AgentEvent::AgentStart
        | AgentEvent::TurnStart
        | AgentEvent::TurnEnd { .. }
        | AgentEvent::MessageEnd { .. } => return Ok(()),
    };
    emitted.map(drop).map_err(thrown)
}

/// The text blocks of a user message joined without a separator.
fn user_message_text(message: &UserMessage) -> String {
    match &message.content {
        UserContent::Text(text) => text.clone(),
        UserContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                maestro_models::UserBlock::Text(text) => Some(text.text.as_str()),
                maestro_models::UserBlock::Image(_) => None,
            })
            .collect(),
    }
}

/// Writes `replacement` into the shared message `target`, which the agent's history, later
/// events, the listeners and the stored entry all read.
fn replace_message_in_place(target: &NativeAgentMessage, replacement: &NativeAgentMessage) {
    match (target, replacement) {
        (AgentMessage::User(target), AgentMessage::User(value)) => copy_into(target, value),
        (AgentMessage::Assistant(target), AgentMessage::Assistant(value)) => {
            copy_into(target, value);
        }
        (AgentMessage::ToolResult(target), AgentMessage::ToolResult(value)) => {
            copy_into(target, value);
        }
        (AgentMessage::Custom(target), AgentMessage::Custom(value)) => copy_into(target, value),
        _ => {}
    }
}

/// Copies the value of `source` into `target` unless both are the same shared message.
fn copy_into<T: Clone>(target: &Arc<std::sync::RwLock<T>>, source: &Arc<std::sync::RwLock<T>>) {
    if !Arc::ptr_eq(target, source) {
        let value = read(source).clone();
        *write(target) = value;
    }
}
