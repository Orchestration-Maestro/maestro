//! Application-owned conversation messages.
use crate::{presence::Presence, types::UserContent};
use serde::{Deserialize, Serialize};

/// Recorded shell execution output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "bashExecution", rename_all = "camelCase")]
pub struct BashExecutionMessage {
    /// Command that ran.
    pub command: String,
    /// Captured output.
    pub output: String,
    /// Supplied exit code.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exit_code: Presence<f64>,
    /// Whether execution was cancelled.
    pub cancelled: bool,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Path to complete output, when supplied.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub full_output_path: Presence<String>,
    /// Recorded time.
    pub timestamp: f64,
    /// Whether the message is excluded from model context.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exclude_from_context: Presence<bool>,
}

/// An extension-injected conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "custom", rename_all = "camelCase")]
#[serde(bound(deserialize = "T: serde::Deserialize<'de>"))]
pub struct CustomMessage<T = serde_json::Value> {
    /// Extension-defined classification.
    pub custom_type: String,
    /// Text or text/image blocks.
    pub content: UserContent,
    /// Whether the message is displayed.
    pub display: bool,
    /// Caller-defined details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
    /// Recorded time.
    pub timestamp: f64,
}

/// Options supplied to a custom-message renderer.
#[derive(Clone, Copy)]
pub struct MessageRenderOptions {
    /// Whether expanded rendering is requested.
    pub expanded: bool,
}

/// A summary of a conversation branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "branchSummary", rename_all = "camelCase")]
pub struct BranchSummaryMessage {
    /// Summary text.
    pub summary: String,
    /// Entry the branch came from.
    pub from_id: String,
    /// Recorded time.
    pub timestamp: f64,
}

/// A summary retained after compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "compactionSummary", rename_all = "camelCase")]
pub struct CompactionSummaryMessage {
    /// Summary text.
    pub summary: String,
    /// Supplied token count before compaction.
    pub tokens_before: f64,
    /// Recorded time.
    pub timestamp: f64,
}

/// The custom-message fields an extension supplies; the host adds the role and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "T: serde::Deserialize<'de>"))]
pub struct CustomMessageInput<T = serde_json::Value> {
    /// Extension-defined classification.
    pub custom_type: String,
    /// Text or text/image blocks.
    pub content: UserContent,
    /// Whether the message is displayed.
    pub display: bool,
    /// Caller-defined details.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<T>,
}
/// Custom-message delivery placement.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageDelivery {
    /// Request steering delivery.
    Steer,
    /// Request follow-up delivery.
    FollowUp,
    /// Request delivery at the next turn.
    NextTurn,
}
/// User-message delivery placement.
///
/// User messages have no next-turn delivery variant:
/// ```compile_fail,E0599
/// let delivery = maestro_request::messages::UserMessageDelivery::NextTurn;
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UserMessageDelivery {
    /// Request steering delivery.
    Steer,
    /// Request follow-up delivery.
    FollowUp,
}
/// Custom-message delivery options; no local defaults are applied.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether the host should trigger a turn.
    pub trigger_turn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional requested delivery placement.
    pub deliver_as: Option<MessageDelivery>,
}
/// User-message delivery options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional requested delivery placement.
    pub deliver_as: Option<UserMessageDelivery>,
}
