//! The single shared event inventory, consumed by schema unions and adapters.
/// Supplies the event inventory to a schema or adapter consumer.
#[macro_export]
macro_rules! extension_event_table {
    ($consumer:ident) => {
        $consumer! {
            before {
                /// Invocation before execution.
                ToolCall(ToolCallEvent),
                /// Completed tool result.
                ToolResult(ToolResultEvent),
                /// User shell command.
                UserBash(UserBashEvent),
                /// Selection notification.
                ModelSelect(Box<ModelSelectEvent>),
                /// Selection notification.
                ThinkingLevelSelect(ThinkingLevelSelectEvent),
                /// Tool execution notification.
                ToolExecutionStart(ToolExecutionStartEvent),
                /// Tool execution notification.
                ToolExecutionUpdate(ToolExecutionUpdateEvent),
                /// Tool execution notification.
                ToolExecutionEnd(ToolExecutionEndEvent),
                /// Context notification.
                Context(Box<ContextEvent>),
                /// `BeforeAgentStart` notification.
                BeforeAgentStart(Box<BeforeAgentStartEvent>),
                /// `AgentStart` notification.
                AgentStart(AgentStartEvent),
                /// `AgentEnd` notification.
                AgentEnd(Box<AgentEndEvent>),
                /// `TurnStart` notification.
                TurnStart(TurnStartEvent),
                /// `TurnEnd` notification.
                TurnEnd(Box<TurnEndEvent>),
                /// `MessageStart` notification.
                MessageStart(Box<MessageStartEvent>),
                /// Stream update notification.
                MessageUpdate(Box<MessageUpdateEvent>),
                /// `MessageEnd` notification.
                MessageEnd(Box<MessageEndEvent>),
                /// Resources are being discovered.
                ResourcesDiscover(ResourcesDiscoverEvent),
            }
            session {
                /// A session started.
                Start / SessionStart(SessionStartEvent, SessionStartEvent),
                /// The session is about to switch.
                BeforeSwitch / SessionBeforeSwitch(SessionBeforeSwitchEvent, SessionBeforeSwitchEvent),
                /// The session is about to fork.
                BeforeFork / SessionBeforeFork(SessionBeforeForkEvent, SessionBeforeForkEvent),
                /// A compaction is about to happen.
                BeforeCompact / SessionBeforeCompact(Box<SessionBeforeCompactEvent>, Box<SessionBeforeCompactEventData>),
                /// A compaction entry was persisted.
                Compact / SessionCompact(SessionCompactEvent, SessionCompactEvent),
                /// Tree navigation is about to occur.
                BeforeTree / SessionBeforeTree(SessionBeforeTreeEvent, SessionBeforeTreeEventData),
                /// Tree navigation completed.
                Tree / SessionTree(SessionTreeEvent, SessionTreeEvent),
                /// A runtime is shutting down.
                Shutdown / SessionShutdown(SessionShutdownEvent, SessionShutdownEvent),
            }
            after {
                /// A provider request is about to be sent.
                BeforeProviderRequest(BeforeProviderRequestEvent),
                /// A provider answered.
                AfterProviderResponse(AfterProviderResponseEvent),
                /// A user input before agent processing.
                Input(InputEvent),
            }
        }
    };
}
