//! What session listeners hear of an agent event while unchecked assistant text, thinking and
//! partial tool output are withheld.
use super::state::SharedAgentEvent;
use crate::utils::lock::read;
use maestro_agent::{AgentEvent, AgentMessage, AgentToolResult};
use maestro_models::{
    AssistantContent, AssistantMessageEvent, SharedAssistantMessage, TextContent, ThinkingContent,
};
use std::sync::{Arc, RwLock};

/// What the listeners hear of `event` while unchecked content is withheld: the event itself,
/// a copy without its unchecked content, or nothing. The start and updates of an assistant
/// message, and a tool's progress, are the only events that carry such content; every other
/// event is heard whole, the message end with the message as the extensions left it.
pub(super) fn heard_while_held(event: &SharedAgentEvent) -> Option<SharedAgentEvent> {
    let held = match &*read(event) {
        AgentEvent::MessageStart {
            message: AgentMessage::Assistant(message),
        } => AgentEvent::MessageStart {
            message: AgentMessage::Assistant(emptied(message)),
        },
        AgentEvent::MessageUpdate {
            message,
            assistant_message_event,
        } => {
            let assistant_message_event = held_model_event(assistant_message_event)?;
            AgentEvent::MessageUpdate {
                message: emptied(message),
                assistant_message_event,
            }
        }
        AgentEvent::ToolExecutionUpdate {
            tool_call,
            partial_result,
        } => AgentEvent::ToolExecutionUpdate {
            tool_call: tool_call.clone(),
            partial_result: AgentToolResult {
                content: Vec::new(),
                details: None,
                terminate: partial_result.terminate,
                usage: None,
            },
        },
        _ => return Some(Arc::clone(event)),
    };
    Some(Arc::new(RwLock::new(held)))
}

/// `event` with the message it carries emptied; none for a text or thinking event, which
/// carries only unchecked content.
fn held_model_event(event: &AssistantMessageEvent) -> Option<AssistantMessageEvent> {
    use AssistantMessageEvent as Event;
    let mut held = event.clone();
    let message = match &mut held {
        Event::TextStart { .. }
        | Event::TextDelta { .. }
        | Event::TextEnd { .. }
        | Event::ThinkingStart { .. }
        | Event::ThinkingDelta { .. }
        | Event::ThinkingEnd { .. } => return None,
        Event::Start { partial }
        | Event::ToolcallStart { partial, .. }
        | Event::ToolcallDelta { partial, .. }
        | Event::ToolcallEnd { partial, .. } => partial,
        Event::Done { message, .. } => message,
        Event::Error { error, .. } => error,
    };
    *message = emptied(message);
    Some(held)
}

/// A copy of `message` whose text and thinking blocks are emptied where they stand, so a tool
/// call's content index still names its block.
fn emptied(message: &SharedAssistantMessage) -> SharedAssistantMessage {
    let mut copy = read(message).clone();
    for block in &mut copy.content {
        match block {
            AssistantContent::Text(text) => *text = TextContent::new(String::new()),
            AssistantContent::Thinking(thinking) => {
                *thinking = ThinkingContent {
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: thinking.redacted,
                };
            }
            AssistantContent::ToolCall(_) => {}
        }
    }
    Arc::new(RwLock::new(copy))
}
