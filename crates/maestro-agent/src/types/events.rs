//! Awaited conversation and tool observations.
use super::{AgentMessage, AgentToolResult, CustomAgentMessages};
pub use maestro_models::ToolCall as AgentToolCall;
use maestro_models::{AssistantMessageEvent, SharedAssistantMessage, ToolResultMessage};
use std::convert::Infallible;

/// An observation from the conversation operation.
pub enum AgentEvent<C: CustomAgentMessages = Infallible> {
    /// The operation begins.
    AgentStart,
    /// The operation completed with its newly admitted messages.
    AgentEnd {
        /// Prompt and generated entries, or only generated entries for continuation.
        messages: Vec<AgentMessage<C>>,
    },
    /// An assistant turn begins.
    TurnStart,
    /// An assistant turn completed.
    TurnEnd {
        /// Authoritative assistant response.
        message: SharedAssistantMessage,
        /// Tool artifacts in call order.
        tool_results: Vec<ToolResultMessage>,
    },
    /// An entry begins.
    MessageStart {
        /// The entry, with an owned assistant snapshot at stream start.
        message: AgentMessage<C>,
    },
    /// A started assistant response changes.
    MessageUpdate {
        /// Owned snapshot of the updated assistant.
        message: SharedAssistantMessage,
        /// Model event retaining its original partial handle.
        assistant_message_event: AssistantMessageEvent,
    },
    /// An entry completed.
    MessageEnd {
        /// Completed shared entry.
        message: AgentMessage<C>,
    },
    /// A tool call begins preparation.
    ToolExecutionStart {
        /// Original unprepared call.
        tool_call: AgentToolCall,
    },
    /// A tool reports progress.
    ToolExecutionUpdate {
        /// Original unprepared call.
        tool_call: AgentToolCall,
        /// Progress output.
        partial_result: AgentToolResult,
    },
    /// A tool completed execution or failed preparation.
    ToolExecutionEnd {
        /// Original unprepared call.
        tool_call: AgentToolCall,
        /// Final output or diagnostic artifact.
        result: AgentToolResult,
        /// Whether preparation or execution failed.
        is_error: bool,
    },
}
