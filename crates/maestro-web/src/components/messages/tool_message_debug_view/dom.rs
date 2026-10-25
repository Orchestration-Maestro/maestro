//! Call and result listings mounted as page content.
use maestro_app::ToolResultMessage;
use std::rc::Rc;

/// Native debug listings, independent of result-payload presence.
#[doc(hidden)]
#[derive(Default)]
pub struct ToolMessageDebugView {
    /// Original optional call arguments; absence displays empty literal code.
    pub call_args: Option<serde_json::Value>,
    /// Optional original result envelope, independent of the visibility flag.
    pub result: Option<Rc<ToolResultMessage>>,
    /// Whether output and details listings are visible.
    pub has_result: bool,
}

#[cfg(target_arch = "wasm32")]
mod browser;
