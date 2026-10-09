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
