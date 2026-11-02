//! Existing extensions owners exposed at the application root.

pub use maestro_extensions::types::{
    ExtensionUIContextPort, ExtensionUIFuture, NotificationType, ThemeSelection, ThemeSetResult,
    UIUnsubscribe,
};

pub use maestro_extensions::{
    AutocompleteProviderFactory, CommandContextOperations, CompactOptions, ContextUsage,
    EventError, ExtensionActions, ExtensionCommandContext, ExtensionCommandContextActions,
    ExtensionContext, ExtensionContextActions, ExtensionError, ExtensionRuntime,
    ExtensionUIContext, ExtensionUIContextBinding, ExtensionUIDialogOptions,
    ExtensionWidgetOptions, ExtensionWidgets, MessageRenderOptions, MessageRenderer,
    ProviderConfig, ProviderModelConfig, TerminalInputHandler, ToolInfo, UIHostCall,
    WidgetPlacement, WorkingIndicatorOptions, create_extension_runtime,
};

pub use maestro_extensions::types::events::*;

/// What a session-changing command reports back: whether it was cancelled.
pub use maestro_extensions::types::SessionChangeResult;

pub use maestro_extensions::types::{ExtensionFlag, ExtensionFlagOptions, FlagType};

/// The option records of the session-changing command operations.
pub use maestro_extensions::types::{ForkOptions, NewSessionCommandOptions, SwitchSessionOptions};
pub use maestro_extensions::{
    Extension, ExtensionAPI, ExtensionExecution, ExtensionFactory, ExtensionHandler,
    ExtensionRunner, RegisteredTool, RegisteredToolInput, load_extension_from_factory,
    wrap_registered_tool, wrap_registered_tools,
};
pub use maestro_models::system_prompt::{BuildSystemPromptOptions, ContextFile};

pub use maestro_extensions::{AgentToolResult, AgentToolUpdateCallback, ToolExecutionMode};

pub use maestro_extensions::MessageRendererCall;

/// The application's bordered loader, as an extension holds it.
pub use maestro_extensions::widgets::{ExtensionEditor, ExtensionKeybindings, ExtensionLoader};

/// Creates an empty shared event bus whose default callbacks use the thread-safe family.
///
/// A frontend binds an inline native renderer to this bus through
/// [`MessageRendererCall::new`] and then retains the projected call.
pub use maestro_extensions::create_event_bus;
pub use maestro_extensions::create_event_bus_for;

pub use maestro_extensions::{
    EventBus, EventBusController, ExtensionShortcut, RegisteredCommand, ResolvedCommand,
};

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::discover_and_load_extensions;

pub use maestro_extensions::footer_data_provider::live::Live;
pub use maestro_extensions::types::{
    ActionHandlers, ComponentFactory, CustomClose, CustomComponentFactory, CustomEditorComponent,
    CustomFactoryOutcome, CustomUIOptions, EditorFactory, EditorFactoryOutcome, ExtensionComponent,
    ExtensionShortcutCallback, FooterFactory, OverlayOptionsSource, WidgetContent,
};

/// Shared identity-preserving callback storage for application consumers.
pub use maestro_listeners::Listeners;
