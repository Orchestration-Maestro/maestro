//! Existing extensions owners exposed at the application root.

pub use maestro_extensions::{
    AutocompleteProviderFactory, CompactOptions, ContextUsage, ExtensionActions,
    ExtensionCommandContext, ExtensionCommandContextActions, ExtensionContext,
    ExtensionContextActions, ExtensionError, ExtensionRuntime, ExtensionUIContext,
    ExtensionUIDialogOptions, ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer,
    ProviderConfig, ProviderModelConfig, TerminalInputHandler, ToolInfo, WidgetPlacement,
    WorkingIndicatorOptions, create_extension_runtime,
};

pub use maestro_extensions::types::events::{
    AfterProviderResponseEvent, AgentEndEvent, AgentStartEvent, BeforeAgentStartEvent,
    BeforeAgentStartEventResult, BeforeProviderRequestEvent, BeforeProviderRequestEventResult,
    ContextEvent, ContextEventResult, CustomToolCallEvent, ExtensionEvent, InputEvent,
    InputEventResult, InputSource, MessageEndEvent, MessageEndEventResult, MessageStartEvent,
    MessageUpdateEvent, ModelSelectEvent, ModelSelectSource, ResourcesDiscoverEvent,
    ResourcesDiscoverResult, SessionBeforeCompactEvent, SessionBeforeCompactResult,
    SessionBeforeForkEvent, SessionBeforeForkResult, SessionBeforeSwitchEvent,
    SessionBeforeSwitchResult, SessionBeforeTreeEvent, SessionBeforeTreeResult,
    SessionCompactEvent, SessionShutdownEvent, SessionStartEvent, SessionTreeEvent,
    ThinkingLevelSelectEvent, ToolCallEvent, ToolCallEventResult, ToolExecutionEndEvent,
    ToolExecutionStartEvent, ToolExecutionUpdateEvent, ToolResultEvent, ToolResultEventResult,
    TurnEndEvent, TurnStartEvent, UserBashEvent, UserBashEventResult, is_bash_tool_result,
    is_edit_tool_result, is_find_tool_result, is_grep_tool_result, is_ls_tool_result,
    is_read_tool_result, is_tool_call_event_type, is_write_tool_result,
};

/// The tool-call events of the native built-in tools.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::types::events::{
    BashToolCallEvent, EditToolCallEvent, FindToolCallEvent, GrepToolCallEvent, LsToolCallEvent,
    ReadToolCallEvent, WriteToolCallEvent,
};

pub use maestro_extensions::types::ExtensionFlag;

pub use maestro_extensions::{
    Extension, ExtensionAPI, ExtensionFactory, ExtensionHandler, ExtensionRunner, RegisteredTool,
    wrap_registered_tool, wrap_registered_tools,
};
pub use maestro_models::system_prompt::BuildSystemPromptOptions;

pub use maestro_extensions::{AgentToolResult, AgentToolUpdateCallback, ToolExecutionMode};

/// Creates an empty shared event bus whose default callbacks use the thread-safe family.
///
/// A frontend binds an inline native renderer to this bus through
/// `maestro_extensions::MessageRendererCall::new` and then retains the projected call.
pub use maestro_extensions::create_event_bus;

pub use maestro_extensions::{
    EventBus, EventBusController, ExtensionShortcut, RegisteredCommand, ResolvedCommand,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::discover_and_load_extensions;
