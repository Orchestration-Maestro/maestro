//! Existing extensions owners exposed at the application root.

pub use maestro_extensions::{
    AutocompleteProviderFactory, CompactOptions, ContextUsage, ExtensionActions,
    ExtensionCommandContext, ExtensionCommandContextActions, ExtensionContext,
    ExtensionContextActions, ExtensionError, ExtensionRuntime, ExtensionUIContext,
    ExtensionUIDialogOptions, ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer,
    ProviderConfig, ProviderModelConfig, TerminalInputHandler, ToolInfo, WidgetPlacement,
    WorkingIndicatorOptions, create_extension_runtime,
};

pub use maestro_extensions::types::events::*;
pub use maestro_extensions::{
    Extension, ExtensionAPI, ExtensionExecution, ExtensionFactory, ExtensionHandler,
    ExtensionRunner, RegisteredTool, RegisteredToolInput, load_extension_from_factory,
    wrap_registered_tool, wrap_registered_tools,
};
pub use maestro_models::system_prompt::{BuildSystemPromptOptions, ContextFile};

pub use maestro_extensions::{AgentToolResult, AgentToolUpdateCallback, ToolExecutionMode};
