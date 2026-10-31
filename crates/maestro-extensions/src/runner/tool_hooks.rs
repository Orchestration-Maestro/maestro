//! Tool interception and caller-owned shell verdicts.
use super::{ExtensionRunner, dispatch::HandlerCursor};
use crate::{
    EventError, ExtensionExecution,
    types::events::{
        ToolCallEvent, ToolCallEventResult, ToolResultEvent, ToolResultEventResult, UserBashEvent,
        UserBashEventResult,
    },
};
use maestro_session::Presence;

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Chains mutable tool-call arguments, stopping on the first explicit block.
    /// # Errors
    /// Propagates the original callback error without diagnostic delivery.
    pub async fn emit_tool_call(
        &self,
        event: &mut ToolCallEvent,
    ) -> Result<Option<ToolCallEventResult>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut result = None;
        while let Some(invocation) = cursor.next::<_, ToolCallEventResult>(self, "tool_call") {
            let Some(next) = (invocation.handler)(event, context.clone()).await? else {
                continue;
            };
            if matches!(next.block, Presence::Present(true)) {
                return Ok(Some(next));
            }
            result = Some(next);
        }
        Ok(result)
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Applies supplied patch fields to a copied result event.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_tool_result(
        &self,
        event: &ToolResultEvent,
    ) -> Result<Option<ToolResultEventResult>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        let mut event = event.clone();
        let mut modified = false;
        while let Some(invocation) = cursor.next::<_, ToolResultEventResult>(self, "tool_result") {
            let Some(result) = invocation.call(self, &mut event, context.clone()).await? else {
                continue;
            };
            if let Presence::Present(content) = result.content {
                event.content = content;
                modified = true;
            }
            if !result.details.is_missing() {
                event.details = result.details;
                modified = true;
            }
            if let Presence::Present(is_error) = result.is_error {
                event.is_error = is_error;
                modified = true;
            }
            if !result.usage.is_missing() {
                event.usage = result.usage;
                modified = true;
            }
        }
        Ok(modified.then_some(ToolResultEventResult {
            content: Presence::Present(event.content),
            details: event.details,
            is_error: Presence::Present(event.is_error),
            usage: event.usage,
        }))
    }
}

impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Returns the first supplied shell verdict, retaining callable operation handles.
    /// # Errors
    /// Returns an error-listener failure.
    pub async fn emit_user_bash<O: 'static, R: 'static>(
        &self,
        event: &mut UserBashEvent,
    ) -> Result<Option<UserBashEventResult<O, R>>, EventError> {
        let context = self.create_context();
        let mut cursor = HandlerCursor::default();
        while let Some(invocation) = cursor.next::<_, UserBashEventResult<O, R>>(self, "user_bash")
        {
            if let Some(result) = invocation.call(self, event, context.clone()).await? {
                return Ok(Some(result));
            }
        }
        Ok(None)
    }
}
