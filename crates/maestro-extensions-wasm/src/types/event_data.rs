//! The JSON data of one event, flat and tagged by its kind, and its conversion to and from the
//! author's event.

use serde::{Deserialize, Serialize};

use super::agent_events::{
    AgentEndEvent, AgentStartEvent, BeforeAgentStartEvent, ContextEvent, MessageEndEvent,
    MessageStartEvent, MessageUpdateEvent, TurnEndEvent, TurnStartEvent,
};
use super::context::AbortSignal;
use super::events::{
    AfterProviderResponseEvent, BeforeProviderRequestEvent, ExtensionEvent, InputEvent,
    ResourcesDiscoverEvent, SessionBeforeCompactEvent, SessionBeforeCompactEventData,
    SessionBeforeForkEvent, SessionBeforeSwitchEvent, SessionEvent, SessionShutdownEvent,
    SessionStartEvent,
};
use super::object;
use super::selection_events::{ModelSelectEvent, ThinkingLevelSelectEvent};
use super::session_events::{
    SessionBeforeTreeEvent, SessionBeforeTreeEventData, SessionCompactEvent, SessionTreeEvent,
};
use super::tool_events::{
    ToolExecutionEndEvent, ToolExecutionStartEvent, ToolExecutionUpdateEvent,
};

/// The `type` tag of an event document.
#[derive(Deserialize)]
#[serde(variant_identifier, rename_all = "snake_case")]
enum Kind {
    /// Selection notification.
    ModelSelect,
    /// Selection notification.
    ThinkingLevelSelect,

    /// Tool execution notification.
    ToolExecutionStart,
    /// Tool execution notification.
    ToolExecutionUpdate,
    /// Tool execution notification.
    ToolExecutionEnd,

    /// Context notification.
    Context,
    /// `BeforeAgentStart` notification.
    BeforeAgentStart,
    /// `AgentStart` notification.
    AgentStart,
    /// `AgentEnd` notification.
    AgentEnd,
    /// `TurnStart` notification.
    TurnStart,
    /// `TurnEnd` notification.
    TurnEnd,
    /// `MessageStart` notification.
    MessageStart,
    /// Stream update notification.
    MessageUpdate,
    /// `MessageEnd` notification.
    MessageEnd,

    /// Resources are being discovered.
    ResourcesDiscover,
    /// A session started.
    SessionStart,
    /// The session is about to switch.
    SessionBeforeSwitch,
    /// The session is about to fork.
    SessionBeforeFork,
    /// A compaction is about to happen.
    SessionBeforeCompact,
    /// A compaction entry was persisted.
    SessionCompact,
    /// Tree navigation is about to occur.
    SessionBeforeTree,
    /// Tree navigation completed.
    SessionTree,
    /// A runtime is shutting down.
    SessionShutdown,
    /// A provider request is about to be sent.
    BeforeProviderRequest,
    /// A provider answered.
    AfterProviderResponse,
    /// A user input before agent processing.
    Input,
}

/// The only property read before the record of the event is.
#[derive(Deserialize)]
struct Tag {
    /// The kind of the event.
    #[serde(rename = "type")]
    kind: Kind,
}

/// The data of one event. Each case holds the single public payload record.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum EventData {
    /// Selection notification.
    ModelSelect(Box<ModelSelectEvent>),
    /// Selection notification.
    ThinkingLevelSelect(ThinkingLevelSelectEvent),

    /// Tool execution notification.
    ToolExecutionStart(ToolExecutionStartEvent),
    /// Tool execution notification.
    ToolExecutionUpdate(ToolExecutionUpdateEvent),
    /// Tool execution notification.
    ToolExecutionEnd(ToolExecutionEndEvent),

    /// Context notification.
    Context(Box<ContextEvent>),
    /// `BeforeAgentStart` notification.
    BeforeAgentStart(Box<BeforeAgentStartEvent>),
    /// `AgentStart` notification.
    AgentStart(AgentStartEvent),
    /// `AgentEnd` notification.
    AgentEnd(Box<AgentEndEvent>),
    /// `TurnStart` notification.
    TurnStart(TurnStartEvent),
    /// `TurnEnd` notification.
    TurnEnd(Box<TurnEndEvent>),
    /// `MessageStart` notification.
    MessageStart(Box<MessageStartEvent>),
    /// Stream update notification.
    MessageUpdate(Box<MessageUpdateEvent>),
    /// `MessageEnd` notification.
    MessageEnd(Box<MessageEndEvent>),

    /// Resources are being discovered.
    ResourcesDiscover(ResourcesDiscoverEvent),
    /// A session started.
    SessionStart(SessionStartEvent),
    /// The session is about to switch.
    SessionBeforeSwitch(SessionBeforeSwitchEvent),
    /// The session is about to fork.
    SessionBeforeFork(SessionBeforeForkEvent),
    /// A compaction is about to happen.
    SessionBeforeCompact(Box<SessionBeforeCompactEventData>),
    /// A compaction entry was persisted.
    SessionCompact(SessionCompactEvent),
    /// Resource-free tree navigation preparation.
    SessionBeforeTree(SessionBeforeTreeEventData),
    /// Tree navigation completed.
    SessionTree(SessionTreeEvent),
    /// A runtime is shutting down.
    SessionShutdown(SessionShutdownEvent),
    /// A provider request is about to be sent.
    BeforeProviderRequest(BeforeProviderRequestEvent),
    /// A provider answered.
    AfterProviderResponse(AfterProviderResponseEvent),
    /// A user input before agent processing.
    Input(InputEvent),
}

impl EventData {
    /// The data a JSON document holds. The tag is read first, with every other property
    /// skipped; the record the tag selects is then read from the same text using its
    /// owning record decoder.
    ///
    /// # Errors
    /// Returns an error when the text is not a JSON object with a recognized string tag,
    /// or when the selected payload's decoder fails. Shared records use their Serde derives
    /// without additional input-shape validation.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        let Tag { kind } = object::from_str(text)?;
        Ok(match kind {
            Kind::ModelSelect => Self::ModelSelect(object::from_str(text)?),
            Kind::ThinkingLevelSelect => Self::ThinkingLevelSelect(object::from_str(text)?),

            Kind::ToolExecutionStart => Self::ToolExecutionStart(object::from_str(text)?),
            Kind::ToolExecutionUpdate => Self::ToolExecutionUpdate(object::from_str(text)?),
            Kind::ToolExecutionEnd => Self::ToolExecutionEnd(object::from_str(text)?),

            Kind::Context => Self::Context(object::from_str(text)?),
            Kind::BeforeAgentStart => Self::BeforeAgentStart(object::from_str(text)?),
            Kind::AgentStart => Self::AgentStart(object::from_str(text)?),
            Kind::AgentEnd => Self::AgentEnd(object::from_str(text)?),
            Kind::TurnStart => Self::TurnStart(object::from_str(text)?),
            Kind::TurnEnd => Self::TurnEnd(object::from_str(text)?),
            Kind::MessageStart => Self::MessageStart(object::from_str(text)?),
            Kind::MessageUpdate => Self::MessageUpdate(object::from_str(text)?),
            Kind::MessageEnd => Self::MessageEnd(object::from_str(text)?),

            Kind::ResourcesDiscover => Self::ResourcesDiscover(object::from_str(text)?),
            Kind::SessionStart => Self::SessionStart(object::from_str(text)?),
            Kind::SessionBeforeSwitch => Self::SessionBeforeSwitch(object::from_str(text)?),
            Kind::SessionBeforeFork => Self::SessionBeforeFork(object::from_str(text)?),
            Kind::SessionBeforeCompact => Self::SessionBeforeCompact(object::from_str(text)?),
            Kind::SessionCompact => Self::SessionCompact(object::from_str(text)?),
            Kind::SessionBeforeTree => Self::SessionBeforeTree(object::from_str(text)?),
            Kind::SessionTree => Self::SessionTree(object::from_str(text)?),
            Kind::SessionShutdown => Self::SessionShutdown(object::from_str(text)?),
            Kind::BeforeProviderRequest => Self::BeforeProviderRequest(object::from_str(text)?),
            Kind::AfterProviderResponse => Self::AfterProviderResponse(object::from_str(text)?),
            Kind::Input => Self::Input(object::from_str(text)?),
        })
    }

    /// The author's event for this data. The signal joins a before-compaction or
    /// before-tree event; every other kind drops it.
    ///
    /// # Errors
    /// Returns an error when a before-compaction or before-tree event has no signal.
    pub(crate) fn attach(self, signal: Option<AbortSignal>) -> Result<ExtensionEvent, String> {
        Ok(match self {
            Self::ModelSelect(event) => ExtensionEvent::ModelSelect(event),
            Self::ThinkingLevelSelect(event) => ExtensionEvent::ThinkingLevelSelect(event),

            Self::ToolExecutionStart(event) => ExtensionEvent::ToolExecutionStart(event),
            Self::ToolExecutionUpdate(event) => ExtensionEvent::ToolExecutionUpdate(event),
            Self::ToolExecutionEnd(event) => ExtensionEvent::ToolExecutionEnd(event),

            Self::Context(event) => ExtensionEvent::Context(event),
            Self::BeforeAgentStart(event) => ExtensionEvent::BeforeAgentStart(event),
            Self::AgentStart(event) => ExtensionEvent::AgentStart(event),
            Self::AgentEnd(event) => ExtensionEvent::AgentEnd(event),
            Self::TurnStart(event) => ExtensionEvent::TurnStart(event),
            Self::TurnEnd(event) => ExtensionEvent::TurnEnd(event),
            Self::MessageStart(event) => ExtensionEvent::MessageStart(event),
            Self::MessageUpdate(event) => ExtensionEvent::MessageUpdate(event),
            Self::MessageEnd(event) => ExtensionEvent::MessageEnd(event),

            Self::ResourcesDiscover(event) => ExtensionEvent::ResourcesDiscover(event),
            Self::SessionStart(event) => ExtensionEvent::Session(SessionEvent::Start(event)),
            Self::SessionBeforeSwitch(event) => {
                ExtensionEvent::Session(SessionEvent::BeforeSwitch(event))
            }
            Self::SessionBeforeFork(event) => {
                ExtensionEvent::Session(SessionEvent::BeforeFork(event))
            }
            Self::SessionBeforeCompact(data) => {
                let signal = signal.ok_or("missing compaction signal")?;
                ExtensionEvent::Session(SessionEvent::BeforeCompact(Box::new(
                    SessionBeforeCompactEvent {
                        data: *data,
                        signal,
                    },
                )))
            }
            Self::SessionCompact(event) => ExtensionEvent::Session(SessionEvent::Compact(event)),
            Self::SessionBeforeTree(data) => {
                let signal = signal.ok_or("missing tree signal")?;
                ExtensionEvent::Session(SessionEvent::BeforeTree(SessionBeforeTreeEvent {
                    preparation: data.preparation,
                    signal,
                }))
            }
            Self::SessionTree(event) => ExtensionEvent::Session(SessionEvent::Tree(event)),
            Self::SessionShutdown(event) => ExtensionEvent::Session(SessionEvent::Shutdown(event)),
            Self::BeforeProviderRequest(event) => ExtensionEvent::BeforeProviderRequest(event),
            Self::AfterProviderResponse(event) => ExtensionEvent::AfterProviderResponse(event),
            Self::Input(event) => ExtensionEvent::Input(event),
        })
    }
}

impl From<ExtensionEvent> for EventData {
    /// The data of an event; an owned cancellation signal is dropped.
    fn from(event: ExtensionEvent) -> Self {
        match event {
            ExtensionEvent::ModelSelect(event) => Self::ModelSelect(event),
            ExtensionEvent::ThinkingLevelSelect(event) => Self::ThinkingLevelSelect(event),

            ExtensionEvent::ToolExecutionStart(event) => Self::ToolExecutionStart(event),
            ExtensionEvent::ToolExecutionUpdate(event) => Self::ToolExecutionUpdate(event),
            ExtensionEvent::ToolExecutionEnd(event) => Self::ToolExecutionEnd(event),

            ExtensionEvent::Context(event) => Self::Context(event),
            ExtensionEvent::BeforeAgentStart(event) => Self::BeforeAgentStart(event),
            ExtensionEvent::AgentStart(event) => Self::AgentStart(event),
            ExtensionEvent::AgentEnd(event) => Self::AgentEnd(event),
            ExtensionEvent::TurnStart(event) => Self::TurnStart(event),
            ExtensionEvent::TurnEnd(event) => Self::TurnEnd(event),
            ExtensionEvent::MessageStart(event) => Self::MessageStart(event),
            ExtensionEvent::MessageUpdate(mut event) => {
                event.snapshot();
                Self::MessageUpdate(event)
            }
            ExtensionEvent::MessageEnd(event) => Self::MessageEnd(event),

            ExtensionEvent::ResourcesDiscover(event) => Self::ResourcesDiscover(event),
            ExtensionEvent::Session(SessionEvent::Start(event)) => Self::SessionStart(event),
            ExtensionEvent::Session(SessionEvent::BeforeSwitch(event)) => {
                Self::SessionBeforeSwitch(event)
            }
            ExtensionEvent::Session(SessionEvent::BeforeFork(event)) => {
                Self::SessionBeforeFork(event)
            }
            ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) => {
                Self::SessionBeforeCompact(Box::new(event.data))
            }
            ExtensionEvent::Session(SessionEvent::Compact(event)) => Self::SessionCompact(event),
            ExtensionEvent::Session(SessionEvent::BeforeTree(event)) => {
                Self::SessionBeforeTree(SessionBeforeTreeEventData {
                    preparation: event.preparation,
                })
            }
            ExtensionEvent::Session(SessionEvent::Tree(event)) => Self::SessionTree(event),
            ExtensionEvent::Session(SessionEvent::Shutdown(event)) => Self::SessionShutdown(event),
            ExtensionEvent::BeforeProviderRequest(event) => Self::BeforeProviderRequest(event),
            ExtensionEvent::AfterProviderResponse(event) => Self::AfterProviderResponse(event),
            ExtensionEvent::Input(event) => Self::Input(event),
        }
    }
}
