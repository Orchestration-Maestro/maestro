//! The JSON data of one event, flat and tagged by its kind, and its conversion to and from the
//! author's event.

use serde::{Deserialize, Serialize};

use super::agent_events::{
    AgentEndEvent, AgentStartEvent, BeforeAgentStartEvent, ContextEvent, MessageEndEvent,
    MessageStartEvent, TurnEndEvent, TurnStartEvent,
};
use super::context::AbortSignal;
use super::events::{
    AfterProviderResponseEvent, BeforeProviderRequestEvent, ExtensionEvent, InputEvent,
    ResourcesDiscoverEvent, SessionBeforeCompactEvent, SessionBeforeCompactEventData,
    SessionBeforeForkEvent, SessionBeforeSwitchEvent, SessionEvent, SessionShutdownEvent,
    SessionStartEvent,
};
use super::object;

/// The `type` tag of an event document.
#[derive(Deserialize)]
#[serde(variant_identifier, rename_all = "snake_case")]
enum Kind {
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
    SessionBeforeCompact(SessionBeforeCompactEventData),
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
            Kind::Context => Self::Context(object::from_str(text)?),
            Kind::BeforeAgentStart => Self::BeforeAgentStart(object::from_str(text)?),
            Kind::AgentStart => Self::AgentStart(object::from_str(text)?),
            Kind::AgentEnd => Self::AgentEnd(object::from_str(text)?),
            Kind::TurnStart => Self::TurnStart(object::from_str(text)?),
            Kind::TurnEnd => Self::TurnEnd(object::from_str(text)?),
            Kind::MessageStart => Self::MessageStart(object::from_str(text)?),
            Kind::MessageEnd => Self::MessageEnd(object::from_str(text)?),

            Kind::ResourcesDiscover => Self::ResourcesDiscover(object::from_str(text)?),
            Kind::SessionStart => Self::SessionStart(object::from_str(text)?),
            Kind::SessionBeforeSwitch => Self::SessionBeforeSwitch(object::from_str(text)?),
            Kind::SessionBeforeFork => Self::SessionBeforeFork(object::from_str(text)?),
            Kind::SessionBeforeCompact => Self::SessionBeforeCompact(object::from_str(text)?),
            Kind::SessionShutdown => Self::SessionShutdown(object::from_str(text)?),
            Kind::BeforeProviderRequest => Self::BeforeProviderRequest(object::from_str(text)?),
            Kind::AfterProviderResponse => Self::AfterProviderResponse(object::from_str(text)?),
            Kind::Input => Self::Input(object::from_str(text)?),
        })
    }

    /// The author's event for this data. The signal joins a compaction; every other kind
    /// drops it.
    ///
    /// # Errors
    /// Returns an error when a compaction has no signal.
    pub(crate) fn attach(self, signal: Option<AbortSignal>) -> Result<ExtensionEvent, String> {
        Ok(match self {
            Self::Context(event) => ExtensionEvent::Context(event),
            Self::BeforeAgentStart(event) => ExtensionEvent::BeforeAgentStart(event),
            Self::AgentStart(event) => ExtensionEvent::AgentStart(event),
            Self::AgentEnd(event) => ExtensionEvent::AgentEnd(event),
            Self::TurnStart(event) => ExtensionEvent::TurnStart(event),
            Self::TurnEnd(event) => ExtensionEvent::TurnEnd(event),
            Self::MessageStart(event) => ExtensionEvent::MessageStart(event),
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
                ExtensionEvent::Session(SessionEvent::BeforeCompact(SessionBeforeCompactEvent {
                    data,
                    signal,
                }))
            }
            Self::SessionShutdown(event) => ExtensionEvent::Session(SessionEvent::Shutdown(event)),
            Self::BeforeProviderRequest(event) => ExtensionEvent::BeforeProviderRequest(event),
            Self::AfterProviderResponse(event) => ExtensionEvent::AfterProviderResponse(event),
            Self::Input(event) => ExtensionEvent::Input(event),
        })
    }
}

impl From<ExtensionEvent> for EventData {
    /// The data of an event; a compaction's signal is dropped.
    fn from(event: ExtensionEvent) -> Self {
        match event {
            ExtensionEvent::Context(event) => Self::Context(event),
            ExtensionEvent::BeforeAgentStart(event) => Self::BeforeAgentStart(event),
            ExtensionEvent::AgentStart(event) => Self::AgentStart(event),
            ExtensionEvent::AgentEnd(event) => Self::AgentEnd(event),
            ExtensionEvent::TurnStart(event) => Self::TurnStart(event),
            ExtensionEvent::TurnEnd(event) => Self::TurnEnd(event),
            ExtensionEvent::MessageStart(event) => Self::MessageStart(event),
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
                Self::SessionBeforeCompact(event.data)
            }
            ExtensionEvent::Session(SessionEvent::Shutdown(event)) => Self::SessionShutdown(event),
            ExtensionEvent::BeforeProviderRequest(event) => Self::BeforeProviderRequest(event),
            ExtensionEvent::AfterProviderResponse(event) => Self::AfterProviderResponse(event),
            ExtensionEvent::Input(event) => Self::Input(event),
        }
    }
}
