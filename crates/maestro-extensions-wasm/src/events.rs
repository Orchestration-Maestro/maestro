use crate::{ToolCallEvent, ToolResultEvent};

/// Narrows a tool-call event by exact name equality, without inspecting input.
pub fn is_tool_call_event_type(tool_name: &str, event: &ToolCallEvent) -> bool {
    event.tool_name == tool_name
}

/// Narrows a bash tool result by exact name equality.
pub fn is_bash_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "bash"
}

/// Narrows a read tool result by exact name equality.
pub fn is_read_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "read"
}

/// Narrows an edit tool result by exact name equality.
pub fn is_edit_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "edit"
}

/// Narrows a write tool result by exact name equality.
pub fn is_write_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "write"
}

/// Narrows a grep tool result by exact name equality.
pub fn is_grep_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "grep"
}

/// Narrows a find tool result by exact name equality.
pub fn is_find_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "find"
}

/// Narrows an ls tool result by exact name equality.
pub fn is_ls_tool_result(event: &ToolResultEvent) -> bool {
    event.tool_name == "ls"
}

/// An owned asynchronous continuation returned after a listener's synchronous prefix.
pub type ListenerTail = crate::ExtensionFuture<'static, ()>;

/// A listener that executes its prefix immediately, optionally returning an owned tail.
pub type EventBusHandler =
    std::rc::Rc<dyn Fn(serde_json::Value) -> Result<Option<ListenerTail>, crate::Error>>;

/// Explicit removal of future listener admission, not cancellation of an existing tail.
pub type Unsubscribe = Box<dyn Fn() -> Result<(), crate::Error>>;

/// Named event communication supplied by the host.
///
/// Emission runs listener bodies and asynchronous prefixes before returning outside
/// synchronous foreign callbacks. Removing a listener does not cancel a tail that
/// has already started. Dropping the removal function does not unsubscribe.
pub trait EventBus {
    /// Emit an arbitrary value on the exact channel name.
    fn emit(&self, channel: &str, data: serde_json::Value) -> Result<(), crate::Error>;
    /// Register one listener and return its explicit removal function.
    fn on(&self, channel: &str, handler: EventBusHandler) -> Result<Unsubscribe, crate::Error>;
}

use crate::{
    AfterProviderResponseEvent, AgentEndEvent, AgentStartEvent, BeforeAgentStartEvent,
    BeforeAgentStartEventResult, BeforeProviderRequestEvent, BeforeProviderRequestEventResult,
    CompactionEntry, ContextEvent, ContextEventResult, InputEvent, InputEventResult,
    MessageEndEvent, MessageEndEventResult, MessageStartEvent, MessageUpdateEvent,
    ModelSelectEvent, ResourcesDiscoverEvent, ResourcesDiscoverResult, SessionBeforeCompactEvent,
    SessionBeforeCompactResult, SessionBeforeForkEvent, SessionBeforeForkResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchResult, SessionBeforeTreeEvent,
    SessionBeforeTreeResult, SessionCompactEvent, SessionShutdownEvent, SessionStartEvent,
    SessionTreeEvent, ThinkingLevelSelectEvent, ToolCallEventResult, ToolExecutionEndEvent,
    ToolExecutionStartEvent, ToolExecutionUpdateEvent, ToolResultEventResult, TurnEndEvent,
    TurnStartEvent, UserBashEvent, UserBashEventResult,
};

/// Events concerning session lifecycle.
pub enum SessionEvent {
    /// The session started.
    SessionStartEvent(SessionStartEvent),
    /// The session is about to switch.
    SessionBeforeSwitchEvent(SessionBeforeSwitchEvent),
    /// The session is about to fork.
    SessionBeforeForkEvent(SessionBeforeForkEvent),
    /// The session is about to compact.
    SessionBeforeCompactEvent(SessionBeforeCompactEvent),
    /// The session compacted.
    SessionCompactEvent(SessionCompactEvent),
    /// The session is shutting down.
    SessionShutdownEvent(SessionShutdownEvent),
    /// The session is about to navigate its tree.
    SessionBeforeTreeEvent(SessionBeforeTreeEvent),
    /// The session navigated its tree.
    SessionTreeEvent(SessionTreeEvent),
}

/// Typed extension callback inputs; arbitrary event names use the raw registration path.
pub enum ExtensionEvent {
    /// Resource discovery.
    ResourcesDiscoverEvent(ResourcesDiscoverEvent),
    /// Session lifecycle.
    SessionEvent(SessionEvent),
    /// Context preparation.
    ContextEvent(ContextEvent),
    /// Provider request preparation.
    BeforeProviderRequestEvent(BeforeProviderRequestEvent),
    /// Provider response metadata.
    AfterProviderResponseEvent(AfterProviderResponseEvent),
    /// Agent preparation.
    BeforeAgentStartEvent(BeforeAgentStartEvent),
    /// Agent start.
    AgentStartEvent(AgentStartEvent),
    /// Agent end.
    AgentEndEvent(AgentEndEvent),
    /// Turn start.
    TurnStartEvent(TurnStartEvent),
    /// Turn end.
    TurnEndEvent(TurnEndEvent),
    /// Message start.
    MessageStartEvent(MessageStartEvent),
    /// Message update.
    MessageUpdateEvent(MessageUpdateEvent),
    /// Message end.
    MessageEndEvent(MessageEndEvent),
    /// Tool execution start.
    ToolExecutionStartEvent(ToolExecutionStartEvent),
    /// Tool execution update.
    ToolExecutionUpdateEvent(ToolExecutionUpdateEvent),
    /// Tool execution end.
    ToolExecutionEndEvent(ToolExecutionEndEvent),
    /// Model selection.
    ModelSelectEvent(Box<ModelSelectEvent>),
    /// Thinking level selection.
    ThinkingLevelSelectEvent(ThinkingLevelSelectEvent),
    /// User bash execution.
    UserBashEvent(UserBashEvent),
    /// User input.
    InputEvent(InputEvent),
    /// Tool call.
    ToolCallEvent(ToolCallEvent),
    /// Tool result.
    ToolResultEvent(ToolResultEvent),
}

/// Optional typed values returned by extension handlers.
pub enum ExtensionEventResult {
    /// Resource discovery paths.
    ResourcesDiscoverResult(ResourcesDiscoverResult),
    /// Context replacement.
    ContextEventResult(ContextEventResult),
    /// Provider request replacement.
    BeforeProviderRequestEventResult(BeforeProviderRequestEventResult),
    /// Input handling.
    InputEventResult(InputEventResult),
    /// Tool call blocking.
    ToolCallEventResult(ToolCallEventResult),
    /// Tool result replacement.
    ToolResultEventResult(ToolResultEventResult),
    /// Message replacement.
    MessageEndEventResult(MessageEndEventResult),
    /// Agent preparation replacement.
    BeforeAgentStartEventResult(BeforeAgentStartEventResult),
    /// Session switching decision.
    SessionBeforeSwitchResult(SessionBeforeSwitchResult),
    /// Fork decision.
    SessionBeforeForkResult(SessionBeforeForkResult),
    /// Compaction replacement.
    SessionBeforeCompactResult(SessionBeforeCompactResult),
    /// Tree navigation replacement.
    SessionBeforeTreeResult(SessionBeforeTreeResult),
    /// User bash replacement.
    UserBashEventResult(UserBashEventResult),
}

impl ResourcesDiscoverEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "resources_discover"
    }
}

impl SessionStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_start"
    }
}

impl SessionBeforeSwitchEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_before_switch"
    }
}

impl SessionBeforeForkEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_before_fork"
    }
}

impl SessionBeforeCompactEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_before_compact"
    }
}

impl SessionCompactEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_compact"
    }
}

impl SessionShutdownEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_shutdown"
    }
}

impl SessionBeforeTreeEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_before_tree"
    }
}

impl SessionTreeEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "session_tree"
    }
}

impl ContextEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "context"
    }
}

impl BeforeProviderRequestEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "before_provider_request"
    }
}

impl AfterProviderResponseEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "after_provider_response"
    }
}

impl BeforeAgentStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "before_agent_start"
    }
}

impl AgentStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "agent_start"
    }
}

impl AgentEndEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "agent_end"
    }
}

impl TurnStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "turn_start"
    }
}

impl TurnEndEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "turn_end"
    }
}

impl MessageStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "message_start"
    }
}

impl MessageEndEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "message_end"
    }
}

impl MessageUpdateEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "message_update"
    }
}

impl ToolExecutionStartEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "tool_execution_start"
    }
}

impl ToolExecutionUpdateEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "tool_execution_update"
    }
}

impl ToolExecutionEndEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "tool_execution_end"
    }
}

impl ModelSelectEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "model_select"
    }
}

impl ThinkingLevelSelectEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "thinking_level_select"
    }
}

impl UserBashEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "user_bash"
    }
}

impl InputEvent {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "input"
    }
}

impl CompactionEntry {
    /// The event's fixed discriminator.
    pub fn event_type(&self) -> &'static str {
        "compaction"
    }
}
