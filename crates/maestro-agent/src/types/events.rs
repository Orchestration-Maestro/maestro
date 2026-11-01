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
/// The event as one record: its snake-case `type`, then its members, with a tool call's
/// identifier, name and arguments as `toolCallId`, `toolName` and `args`.
impl<C: CustomAgentMessages + serde::Serialize> serde::Serialize for AgentEvent<C> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::AgentStart => map.serialize_entry("type", "agent_start")?,
            Self::AgentEnd { messages } => {
                map.serialize_entry("type", "agent_end")?;
                map.serialize_entry("messages", messages)?;
            }
            Self::TurnStart => map.serialize_entry("type", "turn_start")?,
            Self::TurnEnd {
                message,
                tool_results,
            } => {
                map.serialize_entry("type", "turn_end")?;
                map.serialize_entry("message", message)?;
                map.serialize_entry("toolResults", tool_results)?;
            }
            Self::MessageStart { message } => {
                map.serialize_entry("type", "message_start")?;
                map.serialize_entry("message", message)?;
            }
            Self::MessageUpdate {
                message,
                assistant_message_event,
            } => {
                map.serialize_entry("type", "message_update")?;
                map.serialize_entry("assistantMessageEvent", assistant_message_event)?;
                map.serialize_entry("message", message)?;
            }
            Self::MessageEnd { message } => {
                map.serialize_entry("type", "message_end")?;
                map.serialize_entry("message", message)?;
            }
            Self::ToolExecutionStart { tool_call } => {
                serialize_tool_call(&mut map, "tool_execution_start", tool_call)?;
            }
            Self::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } => {
                serialize_tool_call(&mut map, "tool_execution_update", tool_call)?;
                map.serialize_entry("partialResult", partial_result)?;
            }
            Self::ToolExecutionEnd {
                tool_call,
                result,
                is_error,
            } => {
                map.serialize_entry("type", "tool_execution_end")?;
                map.serialize_entry("toolCallId", &tool_call.id)?;
                map.serialize_entry("toolName", &tool_call.name)?;
                map.serialize_entry("result", result)?;
                map.serialize_entry("isError", is_error)?;
            }
        }
        map.end()
    }
}

/// Writes a tool event's `type`, `toolCallId`, `toolName` and `args`.
fn serialize_tool_call<M: serde::ser::SerializeMap>(
    map: &mut M,
    kind: &str,
    tool_call: &AgentToolCall,
) -> Result<(), M::Error> {
    map.serialize_entry("type", kind)?;
    map.serialize_entry("toolCallId", &tool_call.id)?;
    map.serialize_entry("toolName", &tool_call.name)?;
    map.serialize_entry("args", &tool_call.arguments)
}
