//! Ordered assistant content with emitted-position child ownership.
#[cfg(any(target_arch = "wasm32", test))]
use maestro_app::AssistantContent;
use maestro_app::{SharedAssistantMessage, ToolResultMessage};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The actual local collection of executable application tools.
type Tools = Rc<RwLock<Vec<maestro_app::SharedAgentTool<maestro_app::Local>>>>;
/// Original mutable standard result envelopes indexed by exact call ID.
type LiveResults = Rc<HashMap<String, Arc<RwLock<ToolResultMessage>>>>;

/// Original result envelopes, mutable standard records or immutable native details.
pub enum ToolResults<D = serde_json::Value> {
    /// A live result index retaining its original application envelopes.
    Live(LiveResults),
    /// Standalone envelopes without a cloning or serialization bound on their details.
    Native(Rc<HashMap<String, Rc<ToolResultMessage<D>>>>),
}
#[cfg(any(target_arch = "wasm32", test))]
impl<D> ToolResults<D> {
    /// Test presence without reading or copying an envelope.
    fn contains_key(&self, id: &str) -> bool {
        match self {
            Self::Live(results) => results.contains_key(id),
            Self::Native(results) => results.contains_key(id),
        }
    }
}
/// Native assistant row retaining its original application owner.
pub struct AssistantMessage<D = serde_json::Value> {
    /// Required original model response, unset by default.
    pub message: Option<SharedAssistantMessage>,
    /// Available application tools, retained without repeating name lookup.
    pub tools: Option<Tools>,
    /// Exact call IDs still pending.
    pub pending_tool_calls: Option<Rc<HashSet<String>>>,
    /// Hide all tool calls independently of their pending state.
    pub hide_tool_calls: bool,
    /// Result index read only when the selected tool is prepared.
    pub tool_results_by_id: Option<ToolResults<D>>,
    /// Hide usage and shimmer thinking while streaming.
    pub is_streaming: bool,
    /// Hide only pending calls without a paired result.
    pub hide_pending_tool_calls: bool,
    /// Current usage action.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
}
impl<D> Default for AssistantMessage<D> {
    fn default() -> Self {
        Self {
            message: None,
            tools: None,
            pending_tool_calls: None,
            hide_tool_calls: false,
            tool_results_by_id: None,
            is_streaming: false,
            hide_pending_tool_calls: false,
            on_cost_click: None,
        }
    }
}
#[cfg(any(target_arch = "wasm32", test))]
impl<D> AssistantMessage<D> {
    /// Admit each source chunk once without changing its displayed source or searching tools.
    pub(in crate::components::messages) fn parts<'a>(
        &self,
        chunks: impl Iterator<Item = &'a AssistantContent>,
    ) -> impl Iterator<Item = &'a AssistantContent> {
        chunks.filter(|chunk| {
            admitted(chunk, |call| Visibility {
                hide_all: self.hide_tool_calls,
                hide_pending: self.hide_pending_tool_calls,
                pending_without_result: self
                    .pending_tool_calls
                    .as_ref()
                    .is_some_and(|ids| ids.contains(&call.id))
                    && self
                        .tool_results_by_id
                        .as_ref()
                        .is_none_or(|results| !results.contains_key(&call.id)),
            })
        })
    }
}
/// Selected tool visibility, independent of the content representation.
#[cfg(any(target_arch = "wasm32", test))]
struct Visibility {
    /// Hide every call.
    hide_all: bool,
    /// Hide pending calls without results.
    hide_pending: bool,
    /// Whether this exact call is pending without a paired result.
    pending_without_result: bool,
}
/// Apply shared admission without modifying the displayed source.
#[cfg(any(target_arch = "wasm32", test))]
fn admitted(
    chunk: &AssistantContent,
    tool_state: impl FnOnce(&maestro_app::ToolCall) -> Visibility,
) -> bool {
    match chunk {
        AssistantContent::Text(text) => !text.text.chars().all(maestro_app::is_whitespace_scalar),
        AssistantContent::Thinking(text) => {
            !text.thinking.chars().all(maestro_app::is_whitespace_scalar)
        }
        AssistantContent::ToolCall(call) => {
            let state = tool_state(call);
            !(state.hide_all || state.hide_pending && state.pending_without_result)
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
