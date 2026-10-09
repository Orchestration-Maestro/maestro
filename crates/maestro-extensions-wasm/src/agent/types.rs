//! Messages delivered by agent events.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::types::object;
use crate::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage, CustomMessage, Message,
};
/// Agent reasoning levels, including disabled reasoning.
pub use maestro_request::types::ModelThinkingLevel as ThinkingLevel;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};

/// A model message or an extension-defined role.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum AgentMessage {
    /// A model-facing message.
    Message(Box<Message>),
    /// Recorded shell execution.
    BashExecution(BashExecutionMessage),
    /// Extension-injected message.
    Custom(CustomMessage),
    /// Branch summary.
    BranchSummary(BranchSummaryMessage),
    /// Compaction summary.
    CompactionSummary(CompactionSummaryMessage),
    /// Extension-defined data, retaining its opaque text.
    Other(CustomAgentMessages),
}
/// Open extension message data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomAgentMessages {
    /// Authored role.
    pub role: String,
    /// Opaque message data.
    pub data: String,
}
impl<'de> Deserialize<'de> for AgentMessage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Tag {
            /// Selected role.
            role: String,
        }
        let raw = Box::<serde_json::value::RawValue>::deserialize(deserializer)?;
        let tag: Tag = object::from_str(raw.get()).map_err(D::Error::custom)?;
        match tag.role.as_str() {
            "user" | "assistant" | "toolResult" => serde_json::from_str(raw.get())
                .map(Self::Message)
                .map_err(D::Error::custom),
            "bashExecution" => serde_json::from_str(raw.get())
                .map(Self::BashExecution)
                .map_err(D::Error::custom),
            "custom" => serde_json::from_str(raw.get())
                .map(Self::Custom)
                .map_err(D::Error::custom),
            "branchSummary" => serde_json::from_str(raw.get())
                .map(Self::BranchSummary)
                .map_err(D::Error::custom),
            "compactionSummary" => serde_json::from_str(raw.get())
                .map(Self::CompactionSummary)
                .map_err(D::Error::custom),
            _ => object::from_str(raw.get())
                .map(Self::Other)
                .map_err(D::Error::custom),
        }
    }
}
