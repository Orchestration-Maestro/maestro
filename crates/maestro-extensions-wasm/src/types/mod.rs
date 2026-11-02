//! Author types: the API, contexts and events an extension is written against.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod actions;
mod agent_events;
mod agent_results;
mod api;
pub use actions::*;
mod context;
#[cfg(any(test, target_arch = "wasm32"))]
mod event_data;
mod events;
mod extension_result;
pub(crate) mod finite;
pub(crate) mod object;
mod providers;
pub use providers::{ProviderCallbacks, ProviderConfig, ProviderModelConfig};
mod selection_events;
mod stream_options;
pub use stream_options::*;
mod session_events;
mod tool_events;
mod tool_results;
pub use tool_results::*;
mod tools;
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use tools::ToolRow;
pub use tools::*;

pub use agent_events::*;
pub use agent_results::*;
pub use api::{
    ArgumentCompletions, CommandHandler, CommandOptions, ExtensionAPI, ExtensionHost, FlagOptions,
    FlagType, FlagValue, ShortcutHandler, ShortcutOptions,
};
pub use context::{
    AbortListener, AbortSignal, CommandContextPort, CompactOptions, CompactionComplete,
    CompactionError, ContextPort, ContextUsage, ExtensionCommandContext, ExtensionContext,
    ExtensionUIContext, ExtensionUIContextPort, ExtensionUIDialogOptions, ExtensionWidgetOptions,
    ForkData, ForkOptions, NavigateTreeOptions, NewSessionCommandOptions, NotificationType,
    ReplacedSessionContext, ReplacedSessionContextPort, SetupSession, SignalPort,
    SwitchSessionOptions, TerminalInputHandler, TerminalInputResult, WidgetPlacement, WithSession,
    WorkingIndicatorOptions,
};
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) use event_data::EventData;
pub use events::{
    AfterProviderResponseEvent, BeforeProviderRequestEvent, BeforeProviderRequestEventResult,
    ExtensionEvent, ExtensionEventResult, ExtensionHandler, ForkPosition, InputEvent,
    InputEventResult, InputSource, InputTransform, ResourcesDiscoverEvent, ResourcesDiscoverReason,
    ResourcesDiscoverResult, SessionBeforeCompactEvent, SessionBeforeCompactEventData,
    SessionBeforeCompactResult, SessionBeforeForkEvent, SessionBeforeForkResult,
    SessionBeforeSwitchEvent, SessionBeforeSwitchReason, SessionBeforeSwitchResult, SessionEvent,
    SessionShutdownEvent, SessionShutdownReason, SessionStartEvent, SessionStartReason,
};
pub use extension_result::{ExtensionFuture, ExtensionResult};
pub use maestro_request::presence::Presence;
pub use selection_events::*;
pub use session_events::*;
pub use tool_events::*;

pub use crate::bindings::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};

mod renderers;
pub use renderers::*;
