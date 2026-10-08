//! Ordered assistant updates with live shared message handles.
use super::SharedAssistantMessage;
use super::ToolCall;
use serde::{Deserialize, Serialize};
/// Limit successful terminal events to stop, length and tool use.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DoneReason {
    /// Stop.
    Stop,
    /// Length.
    Length,
    /// `ToolUse`.
    ToolUse,
}
/// Limit failed terminal events to abort and error.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorReason {
    /// Aborted.
    Aborted,
    /// Error.
    Error,
}
/// Represent all twelve ordered model event variants with shared message handles.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AssistantMessageEvent {
    /// Start.
    Start {
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `TextStart`.
    TextStart {
        /// Content index.
        content_index: usize,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `TextDelta`.
    TextDelta {
        /// Content index.
        content_index: usize,
        /// Delta.
        delta: String,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `TextEnd`.
    TextEnd {
        /// Content index.
        content_index: usize,
        /// Content.
        content: String,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ThinkingStart`.
    ThinkingStart {
        /// Content index.
        content_index: usize,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ThinkingDelta`.
    ThinkingDelta {
        /// Content index.
        content_index: usize,
        /// Delta.
        delta: String,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ThinkingEnd`.
    ThinkingEnd {
        /// Content index.
        content_index: usize,
        /// Content.
        content: String,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ToolcallStart`.
    ToolcallStart {
        /// Content index.
        content_index: usize,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ToolcallDelta`.
    ToolcallDelta {
        /// Content index.
        content_index: usize,
        /// Delta.
        delta: String,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// `ToolcallEnd`.
    ToolcallEnd {
        /// Content index.
        content_index: usize,
        /// Tool call.
        tool_call: ToolCall,
        /// Partial.
        #[serde(serialize_with = "serialize_shared_message")]
        partial: SharedAssistantMessage,
    },
    /// Done.
    Done {
        /// Reason.
        reason: DoneReason,
        /// Message.
        #[serde(serialize_with = "serialize_shared_message")]
        message: SharedAssistantMessage,
    },
    /// Error.
    Error {
        /// Reason.
        reason: ErrorReason,
        /// Error.
        #[serde(serialize_with = "serialize_shared_message")]
        error: SharedAssistantMessage,
    },
}

/// Serialize the assistant message behind its shared handle.
fn serialize_shared_message<S: serde::Serializer>(
    message: &SharedAssistantMessage,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let snapshot = message
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    snapshot.serialize(serializer)
}
