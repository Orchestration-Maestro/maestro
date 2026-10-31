#![doc = include_str!("../../../docs/extensions/registration.md")]
#![doc = include_str!("../../../docs/extensions/ui.md")]
//! Shared event channels with callback-scoped extension delivery.
//!
//! Supply a serial cooperative spawner that never polls inline or concurrently
//! with the emitting synchronous segment. A suspension must let tails progress.
//!
//! ```
//! use maestro_extensions::create_event_bus;
//! use serde_json::json;
//! use std::sync::{Arc, Mutex};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let runtime = tokio::runtime::Builder::new_current_thread().build()?;
//! runtime.block_on(async {
//!     let controller = create_event_bus(
//!         Arc::new(|tail| { tokio::spawn(tail); }),
//!         Arc::new(|diagnostic| eprint!("{diagnostic}")),
//!     );
//!     let events = controller.for_extension();
//!     let received = Arc::new(Mutex::new(Vec::new()));
//!     let output = Arc::clone(&received);
//!     let subscription = events.on("saved", Arc::new(move |data, _| {
//!         output.lock().map_err(|error| error.to_string())?.push(data);
//!         Ok(None)
//!     }));
//!     events.emit("saved", json!({"path": "notes.md"})).await;
//!     subscription.unsubscribe();
//!     events.emit("saved", json!({"path": "ignored.md"})).await;
//!     assert_eq!(received.lock().unwrap().as_slice(), &[Arc::new(json!({"path": "notes.md"}))]);
//! });
//! # Ok(())
//! # }
//! ```

/// Invocation-local callback emission.
pub mod callbacks;
/// Ordered shared channel delivery.
pub mod event_bus;
pub use callbacks::CallbackEmitter;
pub use event_bus::{
    ErrorReporter, EventBus, EventBusController, EventError, EventListener, EventTail,
    Subscription, TailSpawner, create_event_bus, create_event_bus_for,
};

/// The read-only footer metadata extension footers receive.
pub mod footer_data_provider;
/// Native helper delegation used by component adapters.
#[doc(hidden)]
#[cfg(not(target_arch = "wasm32"))]
pub mod helpers;
mod loader;
/// Native effects supplied to extension adapters.
pub mod native_io;
/// Owned registration records and callback types.
pub mod types;
pub use maestro_catalog::{
    AsyncLockUpdate, AuthCredential, AuthStorage, AuthStorageBackend, AuthStorageData,
    AuthStorageError, AuthStorageFuture, LockUpdate,
};
pub use maestro_catalog::{
    ConfigValueOperations, ConfigValues, ModelRegistry, ResolvedRequestAuth,
};
pub use maestro_models::{
    AssistantMessageEvent, AssistantMessageEventStream, AssistantMessageEventStreamPort, Base,
    BoxFuture, Context, DoneReason, ErrorReason, EventStream, FauxAssistantMessageOptions, Fetch,
    Invoke, OAuthAuthInfo, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthLoginCallbacks,
    OAuthPrompt, OAuthProviderHandle, OAuthProviderInterface, OAuthSelectPrompt, ParameterSchema,
    PayloadHookInput, ProviderCall, ProviderExecution, ProviderInvocation, ProviderResponse,
    ProviderSpawner, ProviderStreamOptions, Refinement, SchemaCallback, SchemaCode, SchemaNode,
    SharedAssistantMessage, SimpleStreamOptions, StopReason, StreamFunction, StreamOptions,
    StreamOptionsInput, Threaded, faux_assistant_message, schema_transport, stream_for,
    stream_simple_for,
};
pub use maestro_resources::SourceInfo;
pub use maestro_session::messages::CustomMessageInput;
pub use maestro_session::session_manager::{
    BranchSummaryOptions, CompactionOptions, ForkPosition, NavigateTreeOptions, NewSessionOptions,
    ReadonlySessionManager, SessionTreeNode,
};
pub use maestro_session::{CompactionResult, Presence, SessionManager};
/// Terminal color conversion mode used by extension themes.
pub use maestro_theme::ColorMode;
pub use maestro_theme::LiveTheme;
/// Prepared terminal colors and text decorations supplied to extension renderers.
pub use maestro_theme::Theme;
/// Background color key, including authored names.
pub use maestro_theme::ThemeBg;
/// Foreground color key, including authored names.
pub use maestro_theme::ThemeColor;
/// What a terminal input listener decided about one chunk of input.
pub use maestro_tui::tui::InputListenerResult;
pub use types::{
    AutocompleteProviderFactory, CommandContextOperations, CompactOptions, ContextUsage,
    EditorFactory, Extension, ExtensionAPI, ExtensionCommandContext,
    ExtensionCommandContextActions, ExtensionContext, ExtensionContextActions, ExtensionExecution,
    ExtensionFlag, ExtensionShortcut, ExtensionUIContext, ExtensionUIContextBinding,
    ExtensionUIDialogOptions, ExtensionWidgetOptions, ExtensionWidgets, ForkHandler,
    MessageRenderOptions, MessageRenderer, MessageRendererCall, NavigateTreeHandler,
    NewSessionHandler, RegisteredCommand, ReplacedSessionContext, ResolvedCommand, ShutdownHandler,
    SwitchSessionHandler, TerminalInputHandler, UIHostCall, WidgetPlacement,
    WorkingIndicatorOptions,
};
/// Queries over current extension registrations.
pub mod runner;
pub use runner::ExtensionRunner;

pub use loader::CatalogConstruction;
#[cfg(not(target_arch = "wasm32"))]
pub use loader::ToolConstruction;
pub use loader::runtime::create_extension_runtime;
pub use maestro_models::{
    Cancellation, Local, Model, ModelThinkingLevel, SharedHandle, ToolInfo,
    system_prompt::BuildSystemPromptOptions,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{ExecOptions, ExecResult};
pub use types::{
    AppendEntryHandler, ExtensionActions, ExtensionError, ExtensionErrorListener, ExtensionRuntime,
    ExtensionRuntimeState, GetActiveToolsHandler, GetAllToolsHandler, GetCommandsHandler,
    GetThinkingLevelHandler, ProviderCallbacks, ProviderConfig, ProviderModelConfig,
    SendMessageHandler, SendUserMessageHandler, SetActiveToolsHandler, SetLabelHandler,
    SetModelHandler, SetThinkingLevelHandler,
};

/// Registered-tool invocation adapters.
pub mod wrapper;
pub use loader::load_extension_from_factory;
#[cfg(not(target_arch = "wasm32"))]
pub use loader::{discover_and_load_extensions, load_extensions, load_extensions_with_runtime};
pub use types::{
    ExtensionFactory, ExtensionHandler, RegisteredTool, RegisteredToolInput, ToolDefinitionMetadata,
};
#[cfg(not(target_arch = "wasm32"))]
pub use types::{ExtensionLoadError, LoadExtensionsResult, ModuleLoader};
pub use wrapper::{wrap_registered_tool, wrap_registered_tools};

pub use types::events::*;

pub use maestro_tools::define_tool;
pub use types::ToolDefinition;

/// Native argument preparation retained by extension definitions.
pub use maestro_agent::PrepareArguments;
pub use maestro_agent::{AgentToolResult, AgentToolUpdateCallback, ToolExecutionMode};
/// Shared extension event inventory for concrete native adapters.
pub use maestro_models::extension_event_table;
/// Native declaration and diagnostic records used by extension adapters.
pub use maestro_models::{DiagnosticCode, DiagnosticErrorInfo, Tool};
pub use maestro_resources::{SlashCommandInfo, SlashCommandSource};
pub use maestro_tools::ToolRenderResultOptions;
/// Native tool framing retained by extension registrations.
pub use maestro_tools::ToolRenderShell;
/// Native per-file mutation ordering shared with built-in edit and write.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::with_file_mutation_queue;
/// Caller-local tool definitions with their original tools-owned types.
pub mod local {
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::edit::local::create_edit_tool_definition;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::process::bash::local::{
        BashSpawnHook, BashToolOptions, create_bash_tool_definition, create_local_bash_operations,
    };
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::read::local::create_read_tool_definition;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::search::find::local::create_find_tool_definition;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::search::grep::local::create_grep_tool_definition;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::search::ls::local::create_ls_tool_definition;
    pub use maestro_tools::tool_definition_wrapper::local::wrap_tool_definition;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::tools::local::{
        ToolDef, ToolsOptions, create_all_tool_definitions, create_coding_tool_definitions,
        create_read_only_tool_definitions, create_tool_definition,
    };
    pub use maestro_tools::types::local::*;
    #[cfg(not(target_arch = "wasm32"))]
    pub use maestro_tools::write::local::create_write_tool_definition;
}
/// Native glob constraints forwarded to replacement find operations.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::search::find::FindGlobOptions;
/// Retained native definitions accepted by the tools-owned execution wrapper.
pub use maestro_tools::tool_definition_wrapper::ToolDefinitionInput;
/// Native raw shell operands and execution family used by returned extension operations.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashError, BashExecOptions, BashExecResult, BashOperations, BashResult, BashSpawnContext,
};
/// Native file configuration and the model observation a read definition consumes.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    EditOperations, EditToolOptions, FindOperations, FindToolOptions, GrepOperations,
    GrepToolOptions, LsOperations, LsToolOptions, ReadContext, ReadOperations, ReadToolOptions,
    SearchTool, ToolRendererBinding, ToolsManager, WriteOperations, WriteToolOptions,
};
/// Native tool renderers, the row context they read and the result they render.
pub use maestro_tools::{
    ToolRenderCall, ToolRenderContext, ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult,
    ToolRenderResultCallback, ToolRendererState, output::render_utils::ToolRenderResultLike,
};
pub use maestro_tui::tui::{Component, ComponentHandle, InputHandler};
/// Toolkit widgets an extension host builds, and the capability that builds the ones whose
/// look depends on the host.
pub mod widgets {
    pub use crate::types::{
        ExtensionEditor, ExtensionKeybindings, ExtensionLoader, ExtensionWidgets,
    };
    pub use maestro_tools::key_text;
    pub use maestro_tui::{
        AutocompleteProvider, AutocompleteSuggestions,
        autocomplete::{CompletionError, CompletionOptions, CompletionResult, CursorPosition},
    };
    pub use maestro_tui::{
        Box, Container, DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, SelectItem,
        SelectList, SelectListLayoutOptions, SelectListTheme, SelectListTruncatePrimaryContext,
        SettingItem, SettingsList, SettingsListTheme, Spacer, Text, TextDecoration,
        components::settings_list::SettingsListOptions,
        editor_component::{BorderColor, EditorComponent, EditorError, TextCallback},
    };
    pub use maestro_tui::{
        Editor, EditorOptions, EditorTheme, FocusFlag, Focusable, KeybindingConflict,
        KeybindingDefinition, KeybindingDefinitions, KeybindingKeys, KeybindingsConfig,
        OverlayAnchor, OverlayHandle, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue,
        get_keybindings,
    };
}
pub use maestro_tui::{AutocompleteItem, Keybinding as AppKeybinding, KeybindingsManager, TUI};
