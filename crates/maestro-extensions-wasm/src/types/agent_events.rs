//! Conversation lifecycle payloads delivered to extension handlers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::object;
use crate::{
    AgentMessage, AssistantMessageEvent, BuildSystemPromptOptions, ImageContent, Presence,
    ToolResultMessage,
};
use serde::{Deserialize, Serialize};
/// Messages presented for context selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEvent {
    /// The messages.
    pub messages: Vec<AgentMessage>,
}

/// Prompt input supplied before the agent starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeAgentStartEvent {
    /// The prompt.
    pub prompt: String,
    /// The images.
    #[serde(
        default,
        skip_serializing_if = "Presence::is_missing",
        deserialize_with = "object::records"
    )]
    pub images: Presence<Vec<ImageContent>>,
    /// The system prompt.
    pub system_prompt: String,
    /// The system prompt options.
    #[serde(deserialize_with = "object::record")]
    pub system_prompt_options: BuildSystemPromptOptions,
}

/// Notification that the agent started.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartEvent {}

/// Messages supplied when the agent ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndEvent {
    /// The messages.
    pub messages: Vec<AgentMessage>,
}

/// The index and time supplied when a turn starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartEvent {
    /// The turn index.
    pub turn_index: f64,
    /// The timestamp.
    pub timestamp: f64,
}

/// The message and tool results supplied when a turn ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEndEvent {
    /// The turn index.
    pub turn_index: f64,
    /// The message.
    pub message: AgentMessage,
    /// The tool results.
    #[serde(deserialize_with = "object::list")]
    pub tool_results: Vec<ToolResultMessage>,
}

/// A message that started.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStartEvent {
    /// The message.
    pub message: AgentMessage,
}

/// A message update with its model stream observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageUpdateEvent {
    /// The agent message, independent of the stream observation.
    pub message: AgentMessage,
    /// The model stream observation.
    pub assistant_message_event: AssistantMessageEvent,
}

impl MessageUpdateEvent {
    /// Detach the shared stream message before output validation and encoding.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn snapshot(&mut self) {
        use AssistantMessageEvent as Event;
        let handle = match &mut self.assistant_message_event {
            Event::Start { partial }
            | Event::TextStart { partial, .. }
            | Event::TextDelta { partial, .. }
            | Event::TextEnd { partial, .. }
            | Event::ThinkingStart { partial, .. }
            | Event::ThinkingDelta { partial, .. }
            | Event::ThinkingEnd { partial, .. }
            | Event::ToolcallStart { partial, .. }
            | Event::ToolcallDelta { partial, .. }
            | Event::ToolcallEnd { partial, .. } => partial,
            Event::Done { message, .. } => message,
            Event::Error { error, .. } => error,
        };
        let snapshot = handle
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        *handle = std::sync::Arc::new(std::sync::RwLock::new(snapshot));
    }
}

/// A message that ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEndEvent {
    /// The message.
    pub message: AgentMessage,
}
