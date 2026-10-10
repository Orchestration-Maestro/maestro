//! Caller initialization and live conversation state.
use crate::types::{AgentMessage, CustomAgentMessages, SharedAgentTool, ThinkingLevel};
use maestro_models::{Model, ModelCost};
use std::{
    convert::Infallible,
    sync::{Arc, OnceLock, RwLock},
};
/// Caller-owned initialization fields; runtime fields cannot be initialized.
///
/// ```compile_fail
/// use maestro_agent::AgentInitialState;
/// let initial: AgentInitialState = AgentInitialState { is_streaming: true, ..Default::default() };
/// ```
pub struct AgentInitialState<C: CustomAgentMessages = Infallible> {
    /// Optional instruction text.
    pub system_prompt: Option<String>,
    /// Optional shared model descriptor.
    pub model: Option<Arc<RwLock<Model>>>,
    /// Optional requested thinking level.
    pub thinking_level: Option<ThinkingLevel>,
    /// Initial executable tools.
    pub tools: Vec<SharedAgentTool>,
    /// Initial conversation entries.
    pub messages: Vec<AgentMessage<C>>,
}
impl<C: CustomAgentMessages> Default for AgentInitialState<C> {
    fn default() -> Self {
        Self {
            system_prompt: None,
            model: None,
            thinking_level: None,
            tools: Vec::new(),
            messages: Vec::new(),
        }
    }
}
/// Live caller fields with private collection slots and runtime observations.
pub struct AgentState<C: CustomAgentMessages = Infallible> {
    /// Instruction text.
    pub system_prompt: String,
    /// Selected shared model descriptor.
    pub model: Arc<RwLock<Model>>,
    /// Requested thinking level.
    pub thinking_level: ThinkingLevel,
    /// Current executable-tool collection.
    tools: Arc<RwLock<Vec<SharedAgentTool>>>,
    /// Current conversation collection.
    pub(super) messages: Arc<RwLock<Vec<AgentMessage<C>>>>,
    /// Runtime streaming flag.
    pub(super) is_streaming: bool,
    /// Runtime partial response.
    pub(super) streaming_message: Option<AgentMessage<C>>,
    /// Runtime pending call ids.
    pub(super) pending_tool_calls: Arc<[String]>,
    /// Runtime error text.
    pub(super) error_message: Option<String>,
}
impl<C: CustomAgentMessages> AgentState<C> {
    /// Construct live slots from caller initialization.
    pub(super) fn new(initial: AgentInitialState<C>) -> Self {
        Self {
            system_prompt: initial.system_prompt.unwrap_or_default(),
            model: initial.model.unwrap_or_else(default_model),
            thinking_level: initial.thinking_level.unwrap_or(ThinkingLevel::Off),
            tools: Arc::new(RwLock::new(initial.tools)),
            messages: Arc::new(RwLock::new(initial.messages)),
            is_streaming: false,
            streaming_message: None,
            pending_tool_calls: Arc::from([]),
            error_message: None,
        }
    }
    /// The current tool collection handle.
    #[must_use]
    pub fn tools(&self) -> &Arc<RwLock<Vec<SharedAgentTool>>> {
        &self.tools
    }
    /// Retain supplied tool entries in new outer storage.
    pub fn set_tools(&mut self, tools: Vec<SharedAgentTool>) {
        self.tools = Arc::new(RwLock::new(tools));
    }
    /// The current history collection handle.
    #[must_use]
    pub fn messages(&self) -> &Arc<RwLock<Vec<AgentMessage<C>>>> {
        &self.messages
    }
    /// Retain supplied message entries in new outer storage.
    pub fn set_messages(&mut self, messages: Vec<AgentMessage<C>>) {
        self.messages = Arc::new(RwLock::new(messages));
    }
    /// Whether a response is currently streaming.
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }
    /// The current partial response, if present.
    #[must_use]
    pub fn streaming_message(&self) -> Option<&AgentMessage<C>> {
        self.streaming_message.as_ref()
    }
    /// The current pending call-id collection.
    #[must_use]
    pub fn pending_tool_calls(&self) -> &Arc<[String]> {
        &self.pending_tool_calls
    }
    /// The current runtime error, if present.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }
}
/// The inert descriptor shared by default agents.
fn default_model() -> Arc<RwLock<Model>> {
    /// Shared descriptor storage.
    static MODEL: OnceLock<Arc<RwLock<Model>>> = OnceLock::new();
    Arc::clone(MODEL.get_or_init(|| {
        Arc::new(RwLock::new(Model {
            id: "unknown".into(),
            name: "unknown".into(),
            api: "unknown".into(),
            provider: "unknown".into(),
            base_url: String::new(),
            reasoning: false,
            thinking_level_map: None,
            input: Vec::new(),
            cost: ModelCost::default(),
            context_window: 0.0,
            max_tokens: 0.0,
            headers: None,
            compat: None,
        }))
    }))
}
