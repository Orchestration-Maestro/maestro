//! Reduce before observing; await registration snapshots without holding locks.
use crate::{AgentEvent, AgentListener, AgentMessage, SubscriptionId, agent::Inner};
use maestro_models::{Cancellation, Message};

pub(crate) async fn emit(inner: &Inner, event: AgentEvent, cancellation: &Cancellation) {
    accept(inner, event).deliver(cancellation).await;
}

pub(crate) struct Delivery {
    event: AgentEvent,
    listeners: Vec<(SubscriptionId, AgentListener)>,
}
impl Delivery {
    pub(crate) async fn deliver(self, cancellation: &Cancellation) {
        for (_, listener) in self.listeners {
            listener(self.event.clone(), cancellation.clone()).await;
        }
    }
}

pub(crate) fn accept(inner: &Inner, event: AgentEvent) -> Delivery {
    let (listeners, streaming) = {
        let mut state = inner.lock();
        match &event {
            AgentEvent::ToolExecutionStart { tool_call_id, .. } => {
                if !state.pending.contains(tool_call_id) {
                    state.pending.push(tool_call_id.clone());
                }
            }
            AgentEvent::ToolExecutionEnd { tool_call_id, .. } => {
                state.pending.retain(|id| id != tool_call_id)
            }
            _ => {}
        }
        let streaming = match &event {
            AgentEvent::MessageStart {
                message: AgentMessage::Model(Message::Assistant(message)),
            } => state.streaming.replace(message.clone()),
            AgentEvent::MessageUpdate { message, .. } => state.streaming.replace(message.clone()),
            AgentEvent::MessageEnd { message } => {
                state.context.messages.push(message.clone());
                if matches!(message, AgentMessage::Model(Message::Assistant(_))) {
                    state.streaming.take()
                } else {
                    None
                }
            }
            _ => None,
        };
        (state.listeners.clone(), streaming)
    };
    drop(streaming);
    Delivery { event, listeners }
}
