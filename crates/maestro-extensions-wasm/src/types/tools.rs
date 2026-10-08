//! Tool registration: the definition an extension registers, its callbacks and the exact
//! name checks over tool events.
use std::rc::Rc;

use serde_json::Value;

use super::context::{AbortSignal, ExtensionContext};
use super::extension_result::{ExtensionFuture, ExtensionResult};
use crate::agent::AgentToolResult;
use crate::bindings::maestro::extension::events::{ToolCallEvent, ToolResultEvent};
pub use crate::bindings::maestro::extension::host::{RenderShell, ToolMetadata};
use crate::event_bus::CallbackEmitter;

/// Rewrites the raw arguments of a tool call before they are validated. It runs
/// synchronously and may emit on the bus through the borrowed emitter.
pub type PrepareArguments =
    Rc<dyn for<'a> Fn(Value, CallbackEmitter<'a>) -> ExtensionResult<Value>>;

/// Reports a partial result of a running tool.
pub type AgentToolUpdateCallback = Rc<dyn Fn(AgentToolResult) -> ExtensionResult<()>>;

/// Runs a tool: receives the call identifier, the prepared arguments, the cancellation
/// signal when there is one, a progress callback when the host listens, and the context.
pub type ToolExecute = Rc<
    dyn Fn(
        String,
        Value,
        Option<AbortSignal>,
        Option<AgentToolUpdateCallback>,
        ExtensionContext,
    ) -> ExtensionFuture<'static, AgentToolResult>,
>;

/// A tool an extension registers for the model to call.
pub struct ToolDefinition {
    /// Name, description, schema and presentation of the tool.
    pub metadata: ToolMetadata,
    /// Rewrites raw arguments before validation, when the tool needs it.
    pub prepare_arguments: Option<PrepareArguments>,
    /// Runs the tool.
    pub execute: ToolExecute,
}

impl ToolCallEvent {
    /// The name of the tool being called.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        match self {
            Self::Bash(_) => "bash",
            Self::Read(_) => "read",
            Self::Edit(_) => "edit",
            Self::Write(_) => "write",
            Self::Grep(_) => "grep",
            Self::Find(_) => "find",
            Self::Ls(_) => "ls",
            Self::Custom(call) => &call.tool_name,
        }
    }
}

impl ToolResultEvent {
    /// The name of the tool that produced the result.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        match self {
            Self::Bash(_) => "bash",
            Self::Read(_) => "read",
            Self::Edit(_) => "edit",
            Self::Write(_) => "write",
            Self::Grep(_) => "grep",
            Self::Find(_) => "find",
            Self::Ls(_) => "ls",
            Self::Custom(result) => &result.tool_name,
        }
    }
}

/// Whether the result is named `bash`; only the name is inspected.
#[must_use]
pub fn is_bash_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "bash"
}

/// Whether the result is named `read`; only the name is inspected.
#[must_use]
pub fn is_read_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "read"
}

/// Whether the result is named `edit`; only the name is inspected.
#[must_use]
pub fn is_edit_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "edit"
}

/// Whether the result is named `write`; only the name is inspected.
#[must_use]
pub fn is_write_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "write"
}

/// Whether the result is named `grep`; only the name is inspected.
#[must_use]
pub fn is_grep_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "grep"
}

/// Whether the result is named `find`; only the name is inspected.
#[must_use]
pub fn is_find_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "find"
}

/// Whether the result is named `ls`; only the name is inspected.
#[must_use]
pub fn is_ls_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name() == "ls"
}

/// Whether the call is for the tool called `tool_name`; the comparison is exact, with no
/// trimming, case folding or normalization.
#[must_use]
pub fn is_tool_call_event_type(tool_name: &str, event: &ToolCallEvent) -> bool {
    event.tool_name() == tool_name
}
