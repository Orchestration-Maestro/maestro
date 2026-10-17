//! Conversation replacements returned by extension handlers.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{AgentMessage, Presence};
use serde::{Deserialize, Serialize};

/// Replacement messages for context selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEventResult {
    /// Supplied replacement messages.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub messages: Presence<Vec<AgentMessage>>,
}

/// Replacement for a finalized message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEndEventResult {
    /// Supplied replacement message.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub message: Presence<AgentMessage>,
}

/// Custom message content returned before the agent starts; details stay opaque JSON text.
pub type BeforeAgentStartMessage = maestro_request::messages::CustomMessageInput<String>;

/// Custom message and system prompt replacements before the agent starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeforeAgentStartEventResult {
    /// Supplied custom message content.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub message: Presence<BeforeAgentStartMessage>,
    /// Supplied replacement system prompt.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub system_prompt: Presence<String>,
}
