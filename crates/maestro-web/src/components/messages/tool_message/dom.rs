//! One tool invocation mounted as page content through the selected renderer.
use maestro_app::{ToolCall, ToolResultMessage};
use std::rc::Rc;

/// Native tool row retaining the original call and result envelope.
#[doc(hidden)]
pub struct ToolMessage<D = serde_json::Value> {
    /// Original model tool call, required when mounted.
    pub tool_call: Option<Rc<ToolCall>>,
    /// Selected tool name; empty or absent selects the call's name.
    pub tool: Option<String>,
    /// Original result envelope with its native details.
    pub result: Option<Rc<ToolResultMessage<D>>>,
    /// Whether execution is pending.
    pub pending: bool,
    /// Replace any result with a synthetic abort.
    pub aborted: bool,
    /// Whether arguments are still streaming.
    pub is_streaming: bool,
}
impl<D> Default for ToolMessage<D> {
    fn default() -> Self {
        Self {
            tool_call: None,
            tool: None,
            result: None,
            pending: false,
            aborted: false,
            is_streaming: false,
        }
    }
}
impl<D> ToolMessage<D> {
    /// Construct a row with its required call unset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
