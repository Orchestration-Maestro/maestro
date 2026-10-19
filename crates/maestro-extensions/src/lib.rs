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
mod loader;
/// Owned registration records and callback types.
pub mod types;
pub use maestro_catalog::{ConfigValueOperations, ModelRegistry, ResolvedRequestAuth};
pub use maestro_resources::SourceInfo;
pub use maestro_session::messages::CustomMessageInput;
pub use maestro_session::session_manager::{
    CompactionOptions, ForkPosition, NavigateTreeOptions, NewSessionOptions,
    ReadonlySessionManager, SessionTreeNode,
};
pub use maestro_session::{CompactionResult, Presence, SessionManager};
pub use maestro_theme::LiveTheme;
pub use types::{
    AutocompleteProviderFactory, CompactOptions, ContextUsage, EditorFactory, Extension,
    ExtensionAPI, ExtensionCommandContext, ExtensionCommandContextActions, ExtensionContext,
    ExtensionContextActions, ExtensionExecution, ExtensionFlag, ExtensionShortcut,
    ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions, ForkHandler,
    MessageRenderOptions, MessageRenderer, NavigateTreeHandler, NewSessionHandler,
    RegisteredCommand, ReplacedSessionContext, ResolvedCommand, ShutdownHandler,
    SwitchSessionHandler, TerminalInputHandler, WidgetPlacement, WorkingIndicatorOptions,
};
/// Queries over current extension registrations.
pub mod runner;
pub use runner::ExtensionRunner;

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
    GetThinkingLevelHandler, ProviderConfig, ProviderModelConfig, SendMessageHandler,
    SendUserMessageHandler, SetActiveToolsHandler, SetLabelHandler, SetModelHandler,
    SetThinkingLevelHandler,
};

/// Registered-tool invocation adapters.
pub mod wrapper;
pub use loader::load_extension_from_factory;
pub use types::{ExtensionFactory, ExtensionHandler, RegisteredTool, RegisteredToolInput};
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
pub use maestro_models::{DiagnosticErrorInfo, Tool};
pub use maestro_resources::{SlashCommandInfo, SlashCommandSource};
pub use maestro_tools::ToolRenderResultOptions;
/// Native tool framing retained by extension registrations.
pub use maestro_tools::ToolRenderShell;
/// Caller-local tool definitions with their original tools-owned types.
pub use maestro_tools::types::local;
/// Native raw shell operands and execution family used by returned extension operations.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{BashError, BashExecOptions, BashExecResult, BashOperations, BashResult};
pub use maestro_tui::tui::{Component, ComponentHandle, InputHandler};
pub use maestro_tui::{AutocompleteItem, Keybinding as AppKeybinding, KeybindingsManager};
