//! Tool execution notifications, without execution policy.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::{Deserialize, Serialize};

/// A tool starts executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionStartEvent {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque arguments text.
    pub args: String,
}

/// Partial output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionUpdateEvent {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque arguments text.
    pub args: String,
    /// Opaque partial result text.
    pub partial_result: String,
}

/// A tool execution finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionEndEvent {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque result text.
    pub result: String,
    /// Whether execution failed.
    pub is_error: bool,
}

use crate::{Presence, UserBlock};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Tool event payload.
pub struct ToolCallEvent<I = ToolInput> {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Input.
    pub input: I,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "D: Deserialize<'de>"))]
/// Tool event payload.
pub struct ToolResultEvent<D = ToolDetails> {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Input.
    pub input: String,
    /// Content.
    pub content: Vec<UserBlock>,
    /// Is error.
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Details.
    pub details: Presence<D>,
}
/// Tool event payload.
pub type CustomToolCallEvent = ToolCallEvent<String>;
/// Tool event payload.
pub type CustomToolResultEvent = ToolResultEvent<String>;
/// Compares the exact tool name.
pub fn is_bash_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "bash"
}
/// Compares the exact tool name.
pub fn is_read_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "read"
}
/// Compares the exact tool name.
pub fn is_edit_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "edit"
}
/// Compares the exact tool name.
pub fn is_write_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "write"
}
/// Compares the exact tool name.
pub fn is_grep_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "grep"
}
/// Compares the exact tool name.
pub fn is_find_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "find"
}
/// Compares the exact tool name.
pub fn is_ls_tool_result<D>(event: &ToolResultEvent<D>) -> bool {
    event.tool_name == "ls"
}
/// Compares the exact tool name.
pub fn is_tool_call_event_type<I>(tool_name: &str, event: &ToolCallEvent<I>) -> bool {
    event.tool_name == tool_name
}

use crate::tools::{
    BashToolDetails, BashToolInput, EditToolDetails, EditToolInput, FindToolDetails, FindToolInput,
    GrepToolDetails, GrepToolInput, LsToolDetails, LsToolInput, ReadToolDetails, ReadToolInput,
    WriteToolInput,
};
/// JSON retained independently of the tool's registered name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolInput(Box<serde_json::value::RawValue>);

impl ToolInput {
    /// Decodes an authored view without changing the retained JSON.
    ///
    /// # Errors
    /// Returns the selected type's JSON decoding error.
    pub fn decode<'de, T: Deserialize<'de>>(&'de self) -> Result<T, serde_json::Error> {
        serde_json::from_str(self.0.get())
    }

    /// Encodes an authored value, rejecting nonfinite numbers before encoding.
    ///
    /// # Errors
    /// Returns an error for nonfinite numbers or failed serialization.
    pub fn from_value<T: Serialize>(value: &T) -> Result<Self, String> {
        let text = crate::types::finite::encode_value(value)?;
        serde_json::value::RawValue::from_string(text)
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

/// Result details use the same JSON carrier as tool input.
pub type ToolDetails = ToolInput;
/// `BashToolCallEvent` data.
pub type BashToolCallEvent = ToolCallEvent<BashToolInput>;
/// `BashToolResultEvent` data.
pub type BashToolResultEvent = ToolResultEvent<BashToolDetails>;
/// `ReadToolCallEvent` data.
pub type ReadToolCallEvent = ToolCallEvent<ReadToolInput>;
/// `ReadToolResultEvent` data.
pub type ReadToolResultEvent = ToolResultEvent<ReadToolDetails>;
/// `EditToolCallEvent` data.
pub type EditToolCallEvent = ToolCallEvent<EditToolInput>;
/// `EditToolResultEvent` data.
pub type EditToolResultEvent = ToolResultEvent<EditToolDetails>;
/// `WriteToolCallEvent` data.
pub type WriteToolCallEvent = ToolCallEvent<WriteToolInput>;
/// `WriteToolResultEvent` data.
pub type WriteToolResultEvent = ToolResultEvent<()>;
/// `GrepToolCallEvent` data.
pub type GrepToolCallEvent = ToolCallEvent<GrepToolInput>;
/// `GrepToolResultEvent` data.
pub type GrepToolResultEvent = ToolResultEvent<GrepToolDetails>;
/// `FindToolCallEvent` data.
pub type FindToolCallEvent = ToolCallEvent<FindToolInput>;
/// `FindToolResultEvent` data.
pub type FindToolResultEvent = ToolResultEvent<FindToolDetails>;
/// `LsToolCallEvent` data.
pub type LsToolCallEvent = ToolCallEvent<LsToolInput>;
/// `LsToolResultEvent` data.
pub type LsToolResultEvent = ToolResultEvent<LsToolDetails>;

#[cfg(any(test, target_arch = "wasm32"))]
use crate::types::object;
#[cfg(any(test, target_arch = "wasm32"))]
impl ToolCallEvent {
    /// Retains the input JSON without selecting a schema by name.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        object::from_str(text)
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
impl ToolResultEvent {
    /// Retains the details JSON without selecting a schema by name.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        object::from_str(text)
    }
}
