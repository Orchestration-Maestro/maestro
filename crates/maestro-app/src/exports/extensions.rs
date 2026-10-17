//! Existing extensions owners exposed at the application root.

pub use maestro_extensions::{
    AutocompleteProviderFactory, CompactOptions, ContextUsage, ExtensionActions,
    ExtensionCommandContext, ExtensionCommandContextActions, ExtensionContext,
    ExtensionContextActions, ExtensionError, ExtensionRuntime, ExtensionUIContext,
    ExtensionUIDialogOptions, ExtensionWidgetOptions, MessageRenderOptions, MessageRenderer,
    ProviderConfig, ProviderModelConfig, TerminalInputHandler, ToolInfo, WidgetPlacement,
    WorkingIndicatorOptions, create_extension_runtime,
};
