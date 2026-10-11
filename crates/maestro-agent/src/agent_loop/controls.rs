//! Queue injection and completed-turn decisions.
use super::{Driver, read, write};
use crate::{
    AgentEvent, AgentMessage, CustomAgentMessages, GetSteeringMessages, ShouldStopAfterTurnContext,
};
use maestro_models::{DiagnosticErrorInfo, SharedAssistantMessage, ToolResultMessage};
use std::sync::{Arc, RwLock};
impl<C: CustomAgentMessages + 'static> Driver<'_, C> {
    /// Await an optional ordered message poll.
    pub(super) async fn poll_messages(
        &self,
        callback: Option<&GetSteeringMessages<C>>,
    ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
        match callback {
            Some(callback) => callback().await,
            None => Ok(Vec::new()),
        }
    }
    /// Select steering before follow-ups, leaving ordinary tool continuation intact.
    pub(super) async fn next_messages(
        &self,
        again: bool,
    ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo> {
        let steering = self
            .poll_messages(self.config.get_steering_messages.as_ref())
            .await?;
        if again || !steering.is_empty() {
            return Ok(steering);
        }
        self.poll_messages(self.config.get_follow_up_messages.as_ref())
            .await
    }
    /// Observe each injected entry before appending its shared handle.
    pub(super) async fn inject_messages(
        &self,
        pending: Vec<AgentMessage<C>>,
        messages: &mut Vec<AgentMessage<C>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        for message in pending {
            (self.emit)(AgentEvent::MessageStart {
                message: message.clone(),
            })
            .await?;
            (self.emit)(AgentEvent::MessageEnd {
                message: message.clone(),
            })
            .await?;
            write(&self.context.messages).push(message.clone());
            messages.push(message);
        }
        Ok(())
    }
    /// Append completed tool handles before awaiting the turn-end observation.
    pub(super) async fn complete_turn(
        &self,
        response: &SharedAssistantMessage,
        results: &[Arc<RwLock<ToolResultMessage>>],
        messages: &mut Vec<AgentMessage<C>>,
    ) -> Result<(), DiagnosticErrorInfo> {
        for result in results {
            let message = AgentMessage::ToolResult(Arc::clone(result));
            write(&self.context.messages).push(message.clone());
            messages.push(message);
        }
        (self.emit)(AgentEvent::TurnEnd {
            message: Arc::clone(response),
            tool_results: results.iter().map(|result| read(result).clone()).collect(),
        })
        .await
    }
    /// Borrow completed records for the optional stop callback.
    pub(super) async fn stop_after_turn(
        &self,
        response: &SharedAssistantMessage,
        results: &[Arc<RwLock<ToolResultMessage>>],
        messages: &[AgentMessage<C>],
    ) -> Result<bool, DiagnosticErrorInfo> {
        match &self.config.should_stop_after_turn {
            Some(stop) => {
                stop(ShouldStopAfterTurnContext {
                    message: response,
                    tool_results: &results
                        .iter()
                        .map(|result| read(result).clone())
                        .collect::<Vec<_>>(),
                    context: &self.context,
                    new_messages: messages,
                })
                .await
            }
            None => Ok(false),
        }
    }
}
