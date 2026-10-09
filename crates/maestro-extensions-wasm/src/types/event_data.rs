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

/// The data of one event. Each case holds the single public payload record.
#[derive(Debug, Serialize, Deserialize)]
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
