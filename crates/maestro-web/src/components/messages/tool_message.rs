//! One tool invocation mounted as page content through the selected renderer.
#[cfg(target_arch = "wasm32")]
use crate::tools::types::{ToolRenderResult, ToolUiError};
use maestro_app::{ToolCall, ToolResultMessage};
use std::rc::Rc;

/// Native tool row retaining the original call and result envelope.
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
/// Dispatch one call to its selected renderer; an abort replaces any result and ends streaming.
/// # Errors
/// Returns reached dispatch or clock failures.
#[cfg(target_arch = "wasm32")]
fn dispatch<D: 'static>(
    props: &ToolMessage<D>,
    call: &ToolCall,
    view: &crate::dom::DomView,
) -> Result<ToolRenderResult, ToolUiError> {
    let aborted = if props.aborted {
        let timestamp = js_sys::Date::now();
        Some(ToolResultMessage {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            content: Vec::new(),
            details: None,
            is_error: true,
            timestamp,
            usage: None,
        })
    } else {
        None
    };
    let name = props
        .tool
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&call.name);
    crate::tools::render_call(
        view,
        name,
        &call.arguments,
        aborted.as_ref().or(props.result.as_deref()),
        !props.aborted && (props.is_streaming || props.pending),
    )
}
#[cfg(target_arch = "wasm32")]
mod browser;
