//! Guest aliases of the shared extension schemas.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

/// Shared schema specialized for the guest operands.
pub type BeforeProviderRequestEvent =
    maestro_request::extension_events::events::BeforeProviderRequestEvent<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type BeforeProviderRequestEventResult =
    maestro_request::extension_events::events::BeforeProviderRequestEventResult<crate::JsonText>;
/// Shared schema specialized for the guest operands.
pub type AfterProviderResponseEvent =
    maestro_request::extension_events::events::AfterProviderResponseEvent<Vec<(String, String)>>;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeCompactEventData =
    maestro_request::extension_events::events::SessionBeforeCompactEventData<
        crate::CompactionPreparation,
        crate::SessionEntry,
    >;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeCompactEvent =
    maestro_request::extension_events::events::SessionBeforeCompactEvent<
        crate::CompactionPreparation,
        crate::SessionEntry,
        crate::AbortSignal,
    >;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeCompactResult =
    maestro_request::extension_events::events::SessionBeforeCompactResult<crate::CompactionResult>;
/// Shared schema specialized for the guest operands.
pub type ResourcesDiscoverReason =
    maestro_request::extension_events::events::ResourcesDiscoverReason;
/// Shared schema specialized for the guest operands.
pub type ResourcesDiscoverEvent = maestro_request::extension_events::events::ResourcesDiscoverEvent;
/// Shared schema specialized for the guest operands.
pub type ResourcesDiscoverResult =
    maestro_request::extension_events::events::ResourcesDiscoverResult;
/// Shared schema specialized for the guest operands.
pub type SessionStartReason = maestro_request::extension_events::events::SessionStartReason;
/// Shared schema specialized for the guest operands.
pub type SessionStartEvent = maestro_request::extension_events::events::SessionStartEvent;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeSwitchReason =
    maestro_request::extension_events::events::SessionBeforeSwitchReason;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeSwitchEvent =
    maestro_request::extension_events::events::SessionBeforeSwitchEvent;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeForkEvent =
    maestro_request::extension_events::events::SessionBeforeForkEvent<ForkPosition>;
/// Shared schema specialized for the guest operands.
pub type SessionShutdownReason = maestro_request::extension_events::events::SessionShutdownReason;
/// Shared schema specialized for the guest operands.
pub type SessionShutdownEvent = maestro_request::extension_events::events::SessionShutdownEvent;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeSwitchResult =
    maestro_request::extension_events::events::SessionBeforeSwitchResult;
/// Shared schema specialized for the guest operands.
pub type SessionBeforeForkResult =
    maestro_request::extension_events::events::SessionBeforeForkResult;
/// Shared schema specialized for the guest operands.
pub type InputSource = maestro_request::extension_events::events::InputSource;
/// Shared schema specialized for the guest operands.
pub type InputEvent = maestro_request::extension_events::events::InputEvent;
/// Shared schema specialized for the guest operands.
pub type InputTransform = maestro_request::extension_events::events::InputTransform;
/// Shared schema specialized for the guest operands.
pub type InputEventResult = maestro_request::extension_events::events::InputEventResult;
/// Shared schema specialized for the guest operands.
pub type SessionEvent = maestro_request::extension_events::unions::SessionEvent<
    SessionBeforeForkEvent,
    SessionBeforeCompactEvent,
    SessionCompactEvent,
    SessionBeforeTreeEvent,
    SessionTreeEvent,
>;
/// Shared schema specialized for the guest operands.
pub type ExtensionEvent = maestro_request::extension_events::unions::ExtensionEvent<
    SessionEvent,
    ContextEvent,
    BeforeAgentStartEvent,
    AgentEndEvent,
    TurnEndEvent,
    MessageStartEvent,
    MessageUpdateEvent,
    MessageEndEvent,
    ToolExecutionStartEvent,
    ToolExecutionUpdateEvent,
    ToolExecutionEndEvent,
    ModelSelectEvent,
    ToolCallEvent,
    ToolResultEvent,
    BeforeProviderRequestEvent,
    AfterProviderResponseEvent,
>;
/// Shared schema specialized for the guest operands.
pub type ExtensionEventResult = maestro_request::extension_events::unions::ExtensionEventResult<
    ContextEventResult,
    MessageEndEventResult,
    BeforeAgentStartEventResult,
    SessionBeforeCompactResult,
    SessionBeforeTreeResult,
    ToolResultEventResult,
    UserBashEventResult,
    BeforeProviderRequestEventResult,
>;
use super::context::ExtensionContext;
use super::extension_result::ExtensionFuture;
use std::rc::Rc;

pub use maestro_entry::session_manager::ForkPosition;
/// Author handler retaining local event operands.
pub type ExtensionHandler = Rc<
    dyn for<'a> Fn(
        &'a mut ExtensionEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<ExtensionEventResult>>,
>;

use crate::{
    AgentEndEvent, BeforeAgentStartEvent, BeforeAgentStartEventResult, ContextEvent,
    ContextEventResult, MessageEndEvent, MessageEndEventResult, MessageStartEvent,
    MessageUpdateEvent, ModelSelectEvent, SessionBeforeTreeEvent, SessionBeforeTreeResult,
    SessionCompactEvent, SessionTreeEvent, ToolCallEvent, ToolExecutionEndEvent,
    ToolExecutionStartEvent, ToolExecutionUpdateEvent, ToolResultEvent, ToolResultEventResult,
    TurnEndEvent, UserBashEventResult,
};
