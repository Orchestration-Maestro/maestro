//! Conversation replacements returned by extension handlers.
use crate::{messages::CustomMessageInput, presence::Presence};
use serde::{Deserialize, Serialize};

/// Replacement messages for context selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "M: Deserialize<'de>"))]
pub struct ContextEventResult<M> {
    /// Supplied replacement messages.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub messages: Presence<Vec<M>>,
}

/// Replacement for a finalized message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "M: Deserialize<'de>"))]
pub struct MessageEndEventResult<M> {
    /// Supplied replacement message.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub message: Presence<M>,
}

/// Custom message content returned before the agent starts; details stay opaque JSON text.
pub type BeforeAgentStartMessage<D> = CustomMessageInput<D>;

/// Custom message and system prompt replacements before the agent starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "M: Deserialize<'de>"))]
pub struct BeforeAgentStartEventResult<M> {
    /// Supplied custom message content.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub message: Presence<M>,
    /// Supplied replacement system prompt.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub system_prompt: Presence<String>,
}
