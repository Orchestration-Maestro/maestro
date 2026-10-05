//! Independent cumulative stream snapshots.

use crate::{AssistantMessage, StopReason, ToolCall};

/// Owned content events with one terminal outcome and immutable retained snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelEvent {
    /// The first valid non-failure update, before content events.
    Start {
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Open an empty text block.
    TextStart {
        /// Stable content-block index.
        content_index: usize,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Append a supplied fragment to an open text block.
    TextDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Close a text block.
    TextEnd {
        /// Stable content-block index.
        content_index: usize,
        /// Completed answer text.
        content: String,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Open empty readable thinking or introduce opaque redacted thinking.
    ThinkingStart {
        /// Stable content-block index.
        content_index: usize,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Append readable reasoning; never carries opaque redacted data.
    ThinkingDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Close thinking; readable content is empty for redacted thinking.
    ThinkingEnd {
        /// Stable content-block index.
        content_index: usize,
        /// Completed readable text; empty for redacted thinking.
        content: String,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Open a tool call with unavailable arguments.
    ToolCallStart {
        /// Stable content-block index.
        content_index: usize,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Append a private JSON fragment without making arguments executable.
    ToolCallDelta {
        /// Stable content-block index.
        content_index: usize,
        /// Exact supplied fragment, including an empty fragment.
        delta: String,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Close a tool call only after strict JSON-object parsing.
    ToolCallEnd {
        /// Stable content-block index.
        content_index: usize,
        /// Completed call with object arguments.
        tool_call: ToolCall,
        /// Independent cumulative response snapshot.
        partial: AssistantMessage,
    },
    /// Successful terminal outcome: Stop, Length or ToolUse only.
    Done {
        /// Terminal outcome.
        reason: StopReason,
        /// Final successful assistant record.
        message: AssistantMessage,
    },
    /// Failed terminal outcome; Cancelled maps to Aborted.
    Error {
        /// Terminal outcome.
        reason: StopReason,
        /// Final failed or aborted assistant record.
        error: AssistantMessage,
    },
}
