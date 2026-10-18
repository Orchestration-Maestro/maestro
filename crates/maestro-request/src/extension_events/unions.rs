//! Shared author-facing unions emitted from the single event inventory.
use super::{
    AgentStartEvent, InputEvent, InputEventResult, ResourcesDiscoverEvent, ResourcesDiscoverResult,
    SessionBeforeForkResult, SessionBeforeSwitchEvent, SessionBeforeSwitchResult,
    SessionShutdownEvent, SessionStartEvent, ThinkingLevelSelectEvent, ToolCallEventResult,
    TurnStartEvent, UserBashEvent,
};
/// Emits generic native/guest unions from the shared inventory.
macro_rules! author_events {
    (before { $($(#[$bm:meta])* $bn:ident($bt:ty),)* }
     session { $($(#[$sm:meta])* $sn:ident / $sd:ident($st:ty, $sdt:ty),)* }
     after { $($(#[$am:meta])* $an:ident($at:ty),)* }) => {
        /// Session records specialized by each capability owner.
        #[derive(Debug)]
        pub enum SessionEvent<SessionBeforeForkEvent,SessionBeforeCompactEvent,SessionCompactEvent,SessionBeforeTreeEvent,SessionTreeEvent> {
            $($(#[$sm])* $sn($st),)*
        }
        /// Complete event records specialized by each capability owner.
        #[derive(Debug)]
        pub enum ExtensionEvent<Session,ContextEvent,BeforeAgentStartEvent,AgentEndEvent,TurnEndEvent,MessageStartEvent,MessageUpdateEvent,MessageEndEvent,ToolExecutionStartEvent,ToolExecutionUpdateEvent,ToolExecutionEndEvent,ModelSelectEvent,ToolCallEvent,ToolResultEvent,BeforeProviderRequestEvent,AfterProviderResponseEvent> {
            $($(#[$bm])* $bn($bt),)*
            /// Session notification.
            Session(Session),
            $($(#[$am])* $an($at),)*
        }
    };
}
crate::extension_event_table!(author_events);
/// Untagged handler verdicts; the invoking event identifies the contract.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum ExtensionEventResult<
    Context,
    MessageEnd,
    BeforeAgent,
    BeforeCompact,
    BeforeTree,
    ToolResult,
    UserBash,
    ProviderRequest,
> {
    /// The `ToolCall` payload.
    ToolCall(ToolCallEventResult),
    /// The `ToolResult` payload.
    ToolResult(ToolResult),
    /// The `UserBash` payload.
    UserBash(UserBash),
    /// The `Context` payload.
    Context(Context),
    /// The `MessageEnd` payload.
    MessageEnd(MessageEnd),
    /// The `BeforeAgentStart` payload.
    BeforeAgentStart(BeforeAgent),
    /// The `ResourcesDiscover` payload.
    ResourcesDiscover(ResourcesDiscoverResult),
    /// The `SessionBeforeSwitch` payload.
    SessionBeforeSwitch(SessionBeforeSwitchResult),
    /// The `SessionBeforeFork` payload.
    SessionBeforeFork(SessionBeforeForkResult),
    /// The `SessionBeforeCompact` payload.
    SessionBeforeCompact(BeforeCompact),
    /// The `SessionBeforeTree` payload.
    SessionBeforeTree(BeforeTree),
    /// The `BeforeProviderRequest` payload.
    BeforeProviderRequest(ProviderRequest),
    /// The `Input` payload.
    Input(InputEventResult),
}
