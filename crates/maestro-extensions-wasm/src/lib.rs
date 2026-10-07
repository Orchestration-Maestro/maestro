//! Author-facing extension data and tool-event narrowing.

mod bindings;
mod contract {
    pub(crate) mod types;
}
mod context;
mod events;
mod extension;

pub use context::*;
pub use contract::types::*;
pub use events::*;
pub use extension::*;

/// A tool call with the bash tool's name.
pub type BashToolCallEvent = ToolCallEvent;
/// A tool call with the read tool's name.
pub type ReadToolCallEvent = ToolCallEvent;
/// A tool call with the edit tool's name.
pub type EditToolCallEvent = ToolCallEvent;
/// A tool call with the write tool's name.
pub type WriteToolCallEvent = ToolCallEvent;
/// A tool call with the grep tool's name.
pub type GrepToolCallEvent = ToolCallEvent;
/// A tool call with the find tool's name.
pub type FindToolCallEvent = ToolCallEvent;
/// A tool call with the ls tool's name.
pub type LsToolCallEvent = ToolCallEvent;
/// A custom tool call.
pub type CustomToolCallEvent = ToolCallEvent;
/// A result with the bash tool's name.
pub type BashToolResultEvent = ToolResultEvent;
/// A result with the read tool's name.
pub type ReadToolResultEvent = ToolResultEvent;
/// A result with the edit tool's name.
pub type EditToolResultEvent = ToolResultEvent;
/// A result with the write tool's name.
pub type WriteToolResultEvent = ToolResultEvent;
/// A result with the grep tool's name.
pub type GrepToolResultEvent = ToolResultEvent;
/// A result with the find tool's name.
pub type FindToolResultEvent = ToolResultEvent;
/// A result with the ls tool's name.
pub type LsToolResultEvent = ToolResultEvent;
/// A custom tool result.
pub type CustomToolResultEvent = ToolResultEvent;
