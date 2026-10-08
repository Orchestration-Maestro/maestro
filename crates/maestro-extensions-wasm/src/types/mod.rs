//! Author types: the API, contexts and events an extension is written against.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod api;
mod context;
mod events;
mod extension_result;
mod tools;

pub use api::{
    ArgumentCompletions, CommandHandler, CommandOptions, ExtensionAPI, ExtensionHost,
    MessageRenderer, ShortcutHandler, ShortcutOptions,
};
pub use context::{
    AbortSignal, BashOperations, BashOperationsPort, CommandContextPort, CompactOptions,
    CompactionComplete, CompactionError, ContextPort, ExtensionCommandContext, ExtensionContext,
    ExtensionUIContext, ExtensionUIContextPort, ForkOptions, NewSessionCommandOptions,
    ReplacedSessionContext, ReplacedSessionContextPort, SetupSession, SignalPort,
    SwitchSessionOptions, Theme, ThemePort, WithSession,
};
pub use events::{
    ExtensionEvent, ExtensionEventResult, ExtensionHandler, SessionBeforeCompactEvent,
    SessionBeforeTreeEvent, SessionEvent, UserBashEventResult,
};
pub use extension_result::{ExtensionFuture, ExtensionResult};
pub use tools::{
    AgentToolUpdateCallback, PrepareArguments, RenderShell, ToolDefinition, ToolExecute,
    ToolMetadata, is_bash_tool_result, is_edit_tool_result, is_find_tool_result,
    is_grep_tool_result, is_ls_tool_result, is_read_tool_result, is_tool_call_event_type,
    is_write_tool_result,
};

pub use crate::bindings::exports::maestro::extension::guest::MessageRenderOptions;
pub use crate::bindings::maestro::extension::events::{
    AfterProviderResponseEvent, AgentEndEvent, BashToolCallEvent, BashToolResultEvent,
    BeforeAgentStartEvent, BeforeAgentStartEventResult, BeforeProviderRequestEvent, ContextEvent,
    ContextEventResult, CustomEvent, CustomToolCallEvent, CustomToolResultEvent, EditToolCallEvent,
    EditToolResultEvent, FindToolCallEvent, FindToolResultEvent, GrepToolCallEvent,
    GrepToolResultEvent, InputEvent, InputEventResult, InputSource, InputTransform,
    LsToolCallEvent, LsToolResultEvent, MessageEndEvent, MessageEndEventResult, MessageStartEvent,
    MessageUpdateEvent, ModelSelectEvent, ModelSelectSource, ReadToolCallEvent,
    ReadToolResultEvent, ResourcesDiscoverEvent, ResourcesDiscoverReason, ResourcesDiscoverResult,
    SessionBeforeCompactEventData, SessionBeforeCompactResult, SessionBeforeForkEvent,
    SessionBeforeForkResult, SessionBeforeSwitchEvent, SessionBeforeSwitchResult,
    SessionBeforeTreeEventData, SessionBeforeTreeResult, SessionCompactEvent, SessionShutdownEvent,
    SessionStartEvent, SessionStartReason, SessionTreeEvent, ShutdownReason, SwitchReason,
    ThinkingLevelSelectEvent, ToolCallEvent, ToolCallEventResult, ToolExecutionEndEvent,
    ToolExecutionStartEvent, ToolExecutionUpdateEvent, ToolResultBase, ToolResultEvent,
    ToolResultEventResult, TreePreparation, TreeSummary, TurnEndEvent, TurnStartEvent,
    UserBashEvent, UserBashEventResultData, WriteToolCallEvent,
};
pub use crate::bindings::maestro::extension::host::{
    FlagOptions, FlagType, FlagValue, MessageDelivery, SendMessageOptions, SendUserMessageOptions,
    UserMessageDelivery,
};
pub use crate::bindings::maestro::extension::session::{
    ContextUsage, ForkData, ForkPosition, NavigateTreeOptions, NewSessionCommandData,
    SessionChangeResult, ToolInfo,
};
