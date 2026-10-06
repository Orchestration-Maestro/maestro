//! Reduce before observing; await registration snapshots without holding locks.
use crate::{AgentEvent, AgentMessage, agent::Inner};
use maestro_models::{Cancellation, Message};

pub(crate) async fn emit(inner: &Inner, event: AgentEvent, cancellation: &Cancellation) {
    let (listeners, streaming) = {
        let mut state = inner.lock();
        let streaming = match &event {
            AgentEvent::MessageStart {
                message: AgentMessage::Model(Message::Assistant(message)),
            } => state
                .streaming
                .replace(crate::run::snapshot_assistant(message)),
            AgentEvent::MessageUpdate { message, .. } => state
                .streaming
                .replace(crate::run::snapshot_assistant(message)),
            AgentEvent::MessageEnd { message } => {
                state
                    .context
                    .messages
                    .push(crate::run::snapshot_record(message));
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
    for (_, listener) in listeners {
        listener(crate::run::snapshot_event(&event), cancellation.clone()).await;
    }
}
