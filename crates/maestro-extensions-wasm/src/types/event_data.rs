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
use super::{ToolCallEvent, ToolResultEvent, UserBashEvent};

/// Emits `Kind`, `EventData` and `EventData::decode` from the event table.
macro_rules! event_data_types {
    (
        before { $($(#[$bm:meta])* $bn:ident($bt:ty),)* }
        session { $($(#[$sm:meta])* $sn:ident / $sd:ident($st:ty, $sdt:ty),)* }
        after { $($(#[$am:meta])* $an:ident($at:ty),)* }
    ) => {
        /// The `type` tag of an event document.
        #[derive(Deserialize)]
        #[serde(variant_identifier, rename_all = "snake_case")]
        enum Kind {
            $($(#[$bm])* $bn,)*
            $($(#[$sm])* $sd,)*
            $($(#[$am])* $an,)*
        }

        /// The data of one event. Each case holds the single public payload record.
        #[derive(Debug, Serialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub(crate) enum EventData {
            $($(#[$bm])* $bn($bt),)*
            $($(#[$sm])* $sd($sdt),)*
            $($(#[$am])* $an($at),)*
        }

        impl EventData {
            /// The data a JSON document holds. The tag is read first, with every other
            /// property skipped; the record the tag selects is then read from the same text
            /// using its owning record decoder.
            ///
            /// # Errors
            /// Returns an error when the text is not a JSON object with a recognized string
            /// tag, or when the selected payload's decoder fails. Shared records use their
            /// Serde derives without additional input-shape validation.
            pub(crate) fn decode(text: &str) -> Result<Self, serde_json::Error> {
                let Tag { kind } = object::from_str(text)?;
                Ok(match kind {
                    $(Kind::$bn => Self::$bn(object::from_str(text)?),)*
                    $(Kind::$sd => Self::$sd(object::from_str(text)?),)*
                    $(Kind::$an => Self::$an(object::from_str(text)?),)*
                })
            }
        }
    };
}

maestro_request::extension_event_table!(event_data_types);

/// The only property read before the record of the event is.
#[derive(Deserialize)]
struct Tag {
    /// The kind of the event.
    #[serde(rename = "type")]
    kind: Kind,
}

impl EventData {
    /// The author's event for this data. The signal joins a before-compaction or
    /// before-tree event; every other kind drops it.
    ///
    /// # Errors
    /// Returns an error when a before-compaction or before-tree event has no signal.
    pub(crate) fn attach(self, signal: Option<AbortSignal>) -> Result<ExtensionEvent, String> {
        Ok(match self {
            Self::ToolCall(event) => ExtensionEvent::ToolCall(event),
            Self::ToolResult(event) => ExtensionEvent::ToolResult(event),
            Self::UserBash(event) => ExtensionEvent::UserBash(event),
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
            ExtensionEvent::ToolCall(event) => Self::ToolCall(event),
            ExtensionEvent::ToolResult(event) => Self::ToolResult(event),
            ExtensionEvent::UserBash(event) => Self::UserBash(event),
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
                snapshot_stream(&mut event);
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

/// Detach one stream observation before output validation.
#[cfg(any(test, target_arch = "wasm32"))]
fn snapshot_stream(event: &mut MessageUpdateEvent) {
    use crate::AssistantMessageEvent as Event;
    let handle = match &mut event.assistant_message_event {
        Event::Start { partial }
        | Event::TextStart { partial, .. }
        | Event::TextDelta { partial, .. }
        | Event::TextEnd { partial, .. }
        | Event::ThinkingStart { partial, .. }
        | Event::ThinkingDelta { partial, .. }
        | Event::ThinkingEnd { partial, .. }
        | Event::ToolcallStart { partial, .. }
        | Event::ToolcallDelta { partial, .. }
        | Event::ToolcallEnd { partial, .. } => partial,
        Event::Done { message, .. } => message,
        Event::Error { error, .. } => error,
    };
    let snapshot = handle
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    *handle = std::sync::Arc::new(std::sync::RwLock::new(snapshot));
}
