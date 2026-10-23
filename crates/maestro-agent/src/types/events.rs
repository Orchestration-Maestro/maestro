//! Awaited conversation and tool observations.
use super::{AgentMessage, AgentToolResult, CustomAgentMessages};
pub use maestro_models::ToolCall as AgentToolCall;
use maestro_models::{AssistantMessageEvent, SharedAssistantMessage, ToolResultMessage};

/// An observation from the conversation operation.
pub enum AgentEvent<C: CustomAgentMessages = crate::NoCustomMessages> {
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
impl<C: CustomAgentMessages> Clone for AgentEvent<C> {
    fn clone(&self) -> Self {
        match self {
            Self::AgentStart => Self::AgentStart,
            Self::AgentEnd { messages } => Self::AgentEnd {
                messages: messages.clone(),
            },
            Self::TurnStart => Self::TurnStart,
            Self::TurnEnd {
                message,
                tool_results,
            } => Self::TurnEnd {
                message: message.clone(),
                tool_results: tool_results.clone(),
            },
            Self::MessageStart { message } => Self::MessageStart {
                message: message.clone(),
            },
            Self::MessageUpdate {
                message,
                assistant_message_event,
            } => Self::MessageUpdate {
                message: message.clone(),
                assistant_message_event: assistant_message_event.clone(),
            },
            Self::MessageEnd { message } => Self::MessageEnd {
                message: message.clone(),
            },
            Self::ToolExecutionStart { tool_call } => Self::ToolExecutionStart {
                tool_call: tool_call.clone(),
            },
            Self::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } => Self::ToolExecutionUpdate {
                tool_call: tool_call.clone(),
                partial_result: partial_result.clone(),
            },
            Self::ToolExecutionEnd {
                tool_call,
                result,
                is_error,
            } => Self::ToolExecutionEnd {
                tool_call: tool_call.clone(),
                result: result.clone(),
                is_error: *is_error,
            },
        }
    }
}
