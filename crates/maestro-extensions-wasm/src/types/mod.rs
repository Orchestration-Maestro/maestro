//! Author types: the API, contexts and events an extension is written against.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod api;
mod context;
#[cfg(any(test, target_arch = "wasm32"))]
mod event_data;
mod events;
mod extension_result;
pub(crate) mod number;
mod presence;

pub use api::{CommandHandler, CommandOptions, ExtensionAPI, ExtensionHost};
pub use context::{
    AbortSignal, CommandContextPort, ContextPort, ExtensionCommandContext, ExtensionContext,
    NewSessionCommandOptions, ReplacedSessionContext, ReplacedSessionContextPort, SignalPort,
    WithSession,
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
pub use presence::Presence;

pub use crate::bindings::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};
