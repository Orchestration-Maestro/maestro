//! Reduce before observing; await registration snapshots without holding locks.
use crate::{AgentEvent, AgentMessage, agent::Inner};
use maestro_models::{Cancellation, Message};

pub(crate) async fn emit(inner: &Inner, event: AgentEvent, cancellation: &Cancellation) {
    let listeners = {
        let mut state = inner.lock();
        match &event {
            AgentEvent::MessageStart {
                message: AgentMessage::Model(Message::Assistant(message)),
            } => state.streaming = Some(message.clone()),
            AgentEvent::MessageUpdate { message, .. } => state.streaming = Some(message.clone()),
            AgentEvent::MessageEnd { message } => {
                state.context.messages.push(message.clone());
                if matches!(message, AgentMessage::Model(Message::Assistant(_))) {
                    state.streaming = None;
                }
            }
            _ => {}
        }
        state.listeners.clone()
    };
    for (_, listener) in listeners {
        listener(event.clone(), cancellation.clone()).await;
    }
}
