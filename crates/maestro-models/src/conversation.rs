//! Owned conversation records, separate from current request instructions.

use crate::{AssistantMessage, InputContent};

/// Timestamped ordered user input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserMessage {
    /// Ordered text and image blocks.
    pub content: Vec<InputContent>,
    /// Supplied Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Caller-owned tool outcome; details are omitted only from outgoing requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolResultMessage {
    /// Originating call identifier.
    pub tool_call_id: String,
    /// Originating tool name.
    pub tool_name: String,
    /// Ordered result content.
    pub content: Vec<InputContent>,
    /// Application data, never provider content.
    pub details: Option<serde_json::Value>,
    /// Whether this result reports an error.
    pub is_error: bool,
    /// Supplied Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Exactly the three conversation record families; instructions remain separate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    /// User input.
    User(UserMessage),
    /// Assistant response or partial attempt.
    Assistant(AssistantMessage),
    /// Tool outcome.
    ToolResult(ToolResultMessage),
}

/// Model-facing declaration, not an executable tool or execution policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDeclaration {
    /// Name matched against tool calls.
    pub name: String,
    /// Readable description.
    pub description: String,
    /// JSON Schema for the argument object.
    pub parameters: serde_json::Value,
}

/// Current instructions and declarations, separate from supplied history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    /// Separate optional current system prompt.
    pub system_prompt: Option<String>,
    /// Ordered caller-owned conversation records.
    pub messages: Vec<Message>,
    /// Current model-facing tool declarations.
    pub tools: Vec<ToolDeclaration>,
}
