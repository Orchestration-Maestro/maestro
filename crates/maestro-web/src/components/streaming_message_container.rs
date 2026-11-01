//! The message being streamed, published at most once per animation frame.
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages, ToolResultMessage};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The live local tools forwarded to the assistant row.
type Tools = Rc<RwLock<Vec<maestro_app::SharedAgentTool<maestro_app::Local>>>>;
/// Original result envelopes by exact call ID.
type LiveResults = Rc<HashMap<String, Arc<RwLock<ToolResultMessage>>>>;
/// One shared transcript entry.
type Message<C> = AgentMessage<BrowserMessages<C>>;

/// The assistant message currently streaming, with a pulse while the response continues.
///
/// `set_message` publishes on the next animation frame, as a copy of the message's content
/// taken then, so several writes in one frame publish once and later edits to the original do
/// not change what was published. Immediate publication and clearing apply at once and retire
/// any queued frame. Only assistant messages are shown; the stable list shows the rest.
pub struct StreamingMessageContainer<C: CustomAgentMessages = NoCustomMessages> {
    /// Local tools forwarded to the assistant row.
    pub tools: Tools,
    /// Whether the response is still streaming; it shows the pulse.
    pub is_streaming: bool,
    /// Calls still waiting for their results.
    pub pending_tool_calls: Option<Rc<HashSet<String>>>,
    /// Live result envelopes forwarded to the assistant row.
    pub tool_results_by_id: Option<LiveResults>,
    /// Usage action forwarded to the assistant row.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
    /// Published and pending messages, the queued frame and the mounted root.
    #[cfg(target_arch = "wasm32")]
    state: Rc<std::cell::RefCell<browser::Stream<C>>>,
    /// Ties the message type to the container on targets without browser state.
    #[cfg(not(target_arch = "wasm32"))]
    message: std::marker::PhantomData<Message<C>>,
}
impl<C: CustomAgentMessages> Default for StreamingMessageContainer<C> {
    fn default() -> Self {
        Self {
            tools: Rc::default(),
            is_streaming: false,
            pending_tool_calls: None,
            tool_results_by_id: None,
            on_cost_click: None,
            #[cfg(target_arch = "wasm32")]
            state: Rc::default(),
            #[cfg(not(target_arch = "wasm32"))]
            message: std::marker::PhantomData,
        }
    }
}
impl<C: CustomAgentMessages + 'static> StreamingMessageContainer<C> {
    /// An idle container with no message.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
