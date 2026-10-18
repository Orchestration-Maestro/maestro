//! Conversation lifecycle payloads delivered to extension handlers.

use super::object;
use crate::system_prompt::BuildSystemPromptOptions;
use crate::{
    presence::Presence,
    types::{ImageContent, ToolResultMessage},
};
use serde::{Deserialize, Serialize};
/// Messages presented for context selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEvent<M> {
    /// The messages.
    pub messages: Vec<M>,
}

/// Prompt input supplied before the agent starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "O: Deserialize<'de>"))]
pub struct BeforeAgentStartEvent<O = BuildSystemPromptOptions> {
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
    pub system_prompt_options: O,
}

/// Notification that the agent started.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartEvent {}

/// Messages supplied when the agent ends.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentEndEvent<M> {
    /// The messages.
    pub messages: Vec<M>,
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
#[serde(bound(deserialize = "M: Deserialize<'de>, T: Deserialize<'de>"))]
pub struct TurnEndEvent<M, T = ToolResultMessage> {
    /// The turn index.
    pub turn_index: f64,
    /// The message.
    pub message: M,
    /// The tool results.
    #[serde(deserialize_with = "object::list")]
    pub tool_results: Vec<T>,
}

/// A message that started.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageStartEvent<M> {
    /// The message.
    pub message: M,
}

/// A message update with its model stream observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageUpdateEvent<M, A> {
    /// The agent message, independent of the stream observation.
    pub message: M,
    /// The model stream observation.
    pub assistant_message_event: A,
}

/// A message that ended.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageEndEvent<M> {
    /// The message.
    pub message: M,
}
