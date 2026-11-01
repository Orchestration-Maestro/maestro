//! Runtime reduction, synthetic failures and final settlement.
use super::{Agent, CustomAgentMessages, lock, read, write};
use crate::types::AgentExecution;
use crate::{AgentEvent, AgentMessage};
use maestro_models::{
    AssistantContent, AssistantMessage, BoxFuture, Cancellation, DiagnosticErrorInfo, StopReason,
    TextContent, Usage, timestamp_now,
};
use std::sync::{Arc, RwLock};
impl<C: CustomAgentMessages + 'static, E: AgentExecution> Agent<C, E> {
    /// The current run signal, absent while idle.
    pub fn signal(&self) -> Option<Cancellation> {
        lock(&self.active).as_ref().map(|run| run.signal.clone())
    }
    /// Signal cooperative cancellation without settling the run or clearing input.
    pub fn abort(&self) {
        if let Some(signal) = self.signal() {
            signal.abort();
        }
    }
    /// Observe the current completion without returning history or propagating failure.
    pub fn wait_for_idle(&self) -> BoxFuture<()> {
        let result = lock(&self.active).as_ref().map(|run| run.result.result());
        Box::pin(async move {
            if let Some(result) = result {
                let _ = result.await;
            }
        })
    }
    /// Publish the failure as an ended assistant message and an ended turn, then the run's
    /// end, each awaiting its listeners in order.
    pub(super) async fn handle_failure(
        agent: E::Shared<Self>,
        error: DiagnosticErrorInfo,
        signal: Cancellation,
    ) -> Result<(), DiagnosticErrorInfo> {
        let failure = {
            let state = read(&agent.state);
            let model = read(&state.model);
            Arc::new(RwLock::new(AssistantMessage {
                content: vec![AssistantContent::Text(TextContent::new(String::new()))],
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                response_model: None,
                response_id: None,
                diagnostics: None,
                usage: Usage::default(),
                stop_reason: if signal.is_aborted() {
                    StopReason::Aborted
                } else {
                    StopReason::Error
                },
                error_message: Some(error.message.clone()),
                timestamp: timestamp_now(),
            }))
        };
        let message = AgentMessage::Assistant(Arc::clone(&failure));
        let events = [
            AgentEvent::MessageStart {
                message: message.clone(),
            },
            AgentEvent::MessageEnd {
                message: message.clone(),
            },
            AgentEvent::TurnEnd {
                message: failure,
                tool_results: Vec::new(),
            },
            AgentEvent::AgentEnd {
                messages: vec![message],
            },
        ];
        for event in events {
            Self::emit(agent.clone(), event, signal.clone()).await?;
        }
        Ok(())
    }
    /// Clear runtime observations and retire the admission after awaited effects.
    pub(super) fn finish(&self) {
        let mut active = lock(&self.active);
        let retired = {
            let mut state = write(&self.state);
            state.is_streaming = false;
            (
                state.streaming_message.take(),
                std::mem::replace(&mut state.pending_tool_calls, Arc::from([])),
            )
        };
        let run = active.take();
        drop(active);
        drop(retired);
        drop(run);
    }
    /// Publish state before listeners, releasing retired entries outside locks.
    pub(super) fn reduce(&self, event: &AgentEvent<C>) {
        let mut state = write(&self.state);
        let mut retired_message = None;
        let mut retired_calls = None;
        let mut retired_error = None;
        match event {
            AgentEvent::MessageStart { message } => {
                retired_message = state.streaming_message.replace(message.clone());
            }
            AgentEvent::MessageUpdate { message, .. } => {
                retired_message = state
                    .streaming_message
                    .replace(AgentMessage::Assistant(Arc::clone(message)));
            }
            AgentEvent::MessageEnd { message } => {
                retired_message = state.streaming_message.take();
                write(state.messages()).push(message.clone());
            }
            AgentEvent::ToolExecutionStart { tool_call } => {
                let mut pending = state.pending_tool_calls.to_vec();
                if !pending.contains(&tool_call.id) {
                    pending.push(tool_call.id.clone());
                }
                retired_calls = Some(std::mem::replace(
                    &mut state.pending_tool_calls,
                    pending.into(),
                ));
            }
            AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                let pending = state
                    .pending_tool_calls
                    .iter()
                    .filter(|id| **id != tool_call.id)
                    .cloned()
                    .collect::<Vec<_>>();
                retired_calls = Some(std::mem::replace(
                    &mut state.pending_tool_calls,
                    pending.into(),
                ));
            }
            AgentEvent::TurnEnd { message, .. } => {
                if let Some(error) = &read(message).error_message
                    && !error.is_empty()
                {
                    retired_error = state.error_message.replace(error.clone());
                }
            }
            AgentEvent::AgentEnd { .. } => retired_message = state.streaming_message.take(),
            _ => (),
        }
        drop(state);
        drop((retired_message, retired_calls, retired_error));
    }
}
