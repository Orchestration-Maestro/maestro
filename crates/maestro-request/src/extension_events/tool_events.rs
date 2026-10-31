//! Tool execution notifications, without execution policy.
use serde::{Deserialize, Serialize};

/// A tool starts executing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionStartEvent<A> {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque arguments.
    pub args: A,
}

/// Partial output from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionUpdateEvent<A, R> {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque arguments.
    pub args: A,
    /// Opaque partial result.
    pub partial_result: R,
}

/// A tool execution finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionEndEvent<R> {
    /// Invocation identifier.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Opaque result.
    pub result: R,
    /// Whether execution failed.
    pub is_error: bool,
}

use crate::{
    presence::Presence,
    types::{Usage, UserBlock},
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Tool event payload.
pub struct ToolCallEvent<I> {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Input.
    pub input: I,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "D: Deserialize<'de>, I: Deserialize<'de>"))]
/// Tool event payload.
pub struct ToolResultEvent<D, I> {
    /// Tool call id.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Input.
    pub input: I,
    /// Content.
    pub content: Vec<UserBlock>,
    /// Is error.
    pub is_error: bool,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Details.
    pub details: Presence<D>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Usage of model calls the tool made itself.
    pub usage: Presence<Usage>,
}
/// Tool event payload.
pub type CustomToolCallEvent<I> = ToolCallEvent<I>;
/// Tool event payload.
pub type CustomToolResultEvent<D, I> = ToolResultEvent<D, I>;
/// Compares the exact tool name.
pub fn is_bash_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "bash"
}
/// Compares the exact tool name.
pub fn is_read_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "read"
}
/// Compares the exact tool name.
pub fn is_edit_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "edit"
}
/// Compares the exact tool name.
pub fn is_write_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "write"
}
/// Compares the exact tool name.
pub fn is_grep_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "grep"
}
/// Compares the exact tool name.
pub fn is_find_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "find"
}
/// Compares the exact tool name.
pub fn is_ls_tool_result<D, I>(event: &ToolResultEvent<D, I>) -> bool {
    event.tool_name == "ls"
}
/// Compares the exact tool name.
pub fn is_tool_call_event_type<I>(tool_name: &str, event: &ToolCallEvent<I>) -> bool {
    event.tool_name == tool_name
}
