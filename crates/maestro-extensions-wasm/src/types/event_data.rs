//! The JSON data of one event, flat and tagged by its kind, and its conversion to and from the
//! author's event.

use serde::{Deserialize, Serialize};

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
    /// skipped; the record the tag selects is then read from the same text, so properties it
    /// does not declare are skipped too, however deeply nested.
    ///
    /// # Errors
    /// Returns an error when the text is malformed, is not a JSON object, has a missing or
    /// unknown tag, or holds a record that is not an object, lacks a required property or
    /// mistypes a property.
    pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
        let Tag { kind } = object::from_str(text)?;
        Ok(match kind {
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
