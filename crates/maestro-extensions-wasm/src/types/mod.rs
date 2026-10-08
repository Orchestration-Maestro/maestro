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

pub use api::{CommandHandler, CommandOptions, ExtensionAPI, ExtensionHost};
pub use context::{
    AbortSignal, CommandContextPort, ContextPort, ExtensionCommandContext, ExtensionContext,
    NewSessionCommandOptions, ReplacedSessionContext, ReplacedSessionContextPort, SignalPort,
    WithSession,
};
pub use events::{
    ExtensionEvent, ExtensionEventResult, ExtensionHandler, SessionBeforeCompactEvent, SessionEvent,
};
pub use extension_result::{ExtensionFuture, ExtensionResult};

pub use crate::bindings::maestro::extension::events::{
    InputEvent, InputEventResult, InputSource, InputTransform, SessionBeforeCompactEventData,
    SessionBeforeCompactResult,
};
pub use crate::bindings::maestro::extension::models::ImageContent;
pub use crate::bindings::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};
