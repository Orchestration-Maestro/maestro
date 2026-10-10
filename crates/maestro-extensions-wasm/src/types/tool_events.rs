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
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
/// `ToolInput` data.
pub enum ToolInput {
    /// Bash payload.
    Bash(BashToolInput),
    /// Read payload.
    Read(ReadToolInput),
    /// Edit payload.
    Edit(EditToolInput),
    /// Write payload.
    Write(WriteToolInput),
    /// Grep payload.
    Grep(GrepToolInput),
    /// Find payload.
    Find(FindToolInput),
    /// Ls payload.
    Ls(LsToolInput),
    /// Custom payload.
    Custom(String),
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
/// `ToolDetails` data.
pub enum ToolDetails {
    /// Bash payload.
    Bash(BashToolDetails),
    /// Read payload.
    Read(ReadToolDetails),
    /// Edit payload.
    Edit(EditToolDetails),
    /// Write payload.
    Write(()),
    /// Grep payload.
    Grep(GrepToolDetails),
    /// Find payload.
    Find(FindToolDetails),
    /// Ls payload.
    Ls(LsToolDetails),
    /// Custom payload.
    Custom(String),
}
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
use serde_json::value::RawValue;
#[cfg(any(test, target_arch = "wasm32"))]
impl ToolCallEvent {
    /// Decodes the payload selected by the exact tool name.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        let event: ToolCallEvent<Box<RawValue>> = object::from_str(text)?;
        let raw = event.input.get();
        let input = match event.tool_name.as_str() {
            "bash" => ToolInput::Bash(serde_json::from_str(raw)?),
            "read" => ToolInput::Read(serde_json::from_str(raw)?),
            "edit" => ToolInput::Edit(serde_json::from_str(raw)?),
            "write" => ToolInput::Write(serde_json::from_str(raw)?),
            "grep" => ToolInput::Grep(serde_json::from_str(raw)?),
            "find" => ToolInput::Find(serde_json::from_str(raw)?),
            "ls" => ToolInput::Ls(serde_json::from_str(raw)?),
            _ => ToolInput::Custom(serde_json::from_str(raw)?),
        };
        Ok(Self {
            tool_call_id: event.tool_call_id,
            tool_name: event.tool_name,
            input,
        })
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
impl ToolResultEvent {
    /// Decodes the payload selected by the exact tool name.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        let event: ToolResultEvent<Box<RawValue>> = object::from_str(text)?;
        let details = match event.details {
            Presence::Missing => Presence::Missing,
            Presence::Null => Presence::Null,
            Presence::Present(raw) => Presence::Present(match event.tool_name.as_str() {
                "bash" => ToolDetails::Bash(serde_json::from_str(raw.get())?),
                "read" => ToolDetails::Read(serde_json::from_str(raw.get())?),
                "edit" => ToolDetails::Edit(serde_json::from_str(raw.get())?),
                "write" => ToolDetails::Write(serde_json::from_str(raw.get())?),
                "grep" => ToolDetails::Grep(serde_json::from_str(raw.get())?),
                "find" => ToolDetails::Find(serde_json::from_str(raw.get())?),
                "ls" => ToolDetails::Ls(serde_json::from_str(raw.get())?),
                _ => ToolDetails::Custom(serde_json::from_str(raw.get())?),
            }),
        };
        Ok(Self {
            tool_call_id: event.tool_call_id,
            tool_name: event.tool_name,
            input: event.input,
            content: event.content,
            is_error: event.is_error,
            details,
        })
    }
}
