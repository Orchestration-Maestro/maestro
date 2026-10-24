//! Message and provider-payload replacement chains.
use super::{ExtensionRunner, dispatch::HandlerCursor};
use crate::types::read;
use crate::{
    EventError, ExtensionExecution,
    types::events::{
        BeforeProviderRequestEvent, BeforeProviderRequestEventResult, ContextEvent,
        ContextEventResult, MessageEndEvent, MessageEndEventResult,
    },
};
use maestro_agent::{AgentMessage, CustomAgentMessages};
use maestro_session::Presence;
use serde_json::Value;
use std::sync::{Arc, RwLock};

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Chains every supplied payload value, including JSON null.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_before_provider_request(&self, payload: Value) -> Result<Value, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut event = BeforeProviderRequestEvent { payload };
        while let Some(invocation) =
            cursor.next::<_, BeforeProviderRequestEventResult>(self, "before_provider_request")
        {
            if let Some(payload) = invocation.call(self, &mut event, context.clone()).await? {
                event.payload = payload;
            }
        }
        Ok(event.payload)
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Copies initial message payloads once, then chains mutable edits and replacements.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_context<C: CustomAgentMessages + Clone + 'static>(
        &self,
        messages: &[AgentMessage<C>],
    ) -> Result<Vec<AgentMessage<C>>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut event = ContextEvent {
            messages: messages.iter().map(copy_message).collect(),
        };
        while let Some(invocation) =
            cursor.next::<_, ContextEventResult<AgentMessage<C>>>(self, "context")
        {
            if let Some(ContextEventResult {
                messages: Presence::Present(messages),
            }) = invocation.call(self, &mut event, context.clone()).await?
            {
                event.messages = messages;
            }
        }
        Ok(event.messages)
    }
}
/// Copies the payload snapshot, not the shared ownership handle.
fn copy_payload<T: Clone>(payload: &Arc<RwLock<T>>) -> Arc<RwLock<T>> {
    Arc::new(RwLock::new(read(payload).clone()))
}
/// Preserves concrete message variants while copying their underlying records.
fn copy_message<C: CustomAgentMessages + Clone>(message: &AgentMessage<C>) -> AgentMessage<C> {
    match message {
        AgentMessage::User(value) => AgentMessage::User(copy_payload(value)),
        AgentMessage::Assistant(value) => AgentMessage::Assistant(copy_payload(value)),
        AgentMessage::ToolResult(value) => AgentMessage::ToolResult(copy_payload(value)),
        AgentMessage::Custom(value) => AgentMessage::Custom(copy_payload(value)),
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Chains same-role message replacements; even an identical handle marks a change.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_message_end<C: CustomAgentMessages + 'static>(
        &self,
        event: &mut MessageEndEvent<AgentMessage<C>>,
    ) -> Result<Option<AgentMessage<C>>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut current = event.message.clone();
        let mut modified = false;
        while let Some(invocation) =
            cursor.next::<_, MessageEndEventResult<AgentMessage<C>>>(self, "message_end")
        {
            let mut event = MessageEndEvent {
                message: current.clone(),
            };
            let Some(MessageEndEventResult {
                message: Presence::Present(message),
            }) = invocation.call(self, &mut event, context.clone()).await?
            else {
                continue;
            };
            if same_role(&message, &current) {
                current = message;
                modified = true;
            } else {
                let extension_path = read(&invocation.owner).path.clone();
                self.emit_error(&crate::ExtensionError {
                    extension_path,
                    event: "message_end".into(),
                    error: "message_end handlers must return a message with the same role".into(),
                    stack: None,
                })?;
            }
        }
        Ok(modified.then_some(current))
    }
}
/// Borrows a concrete role without cloning the payload or allocating a role string.
fn with_role<C: CustomAgentMessages, R>(
    message: &AgentMessage<C>,
    inspect: impl FnOnce(&str) -> R,
) -> R {
    match message {
        AgentMessage::User(_) => inspect("user"),
        AgentMessage::Assistant(_) => inspect("assistant"),
        AgentMessage::ToolResult(_) => inspect("toolResult"),
        AgentMessage::Custom(value) => inspect(read(value).role()),
    }
}
/// Compares actual role values, avoiding a repeated read lock for one custom owner.
fn same_role<C: CustomAgentMessages>(
    candidate: &AgentMessage<C>,
    current: &AgentMessage<C>,
) -> bool {
    if let (AgentMessage::Custom(candidate), AgentMessage::Custom(current)) = (candidate, current)
        && Arc::ptr_eq(candidate, current)
    {
        return true;
    }
    with_role(candidate, |role| {
        with_role(current, |current| role == current)
    })
}
