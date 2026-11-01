//! The shared event inventory selects concrete native handlers, never a union surrogate.
use crate::{
    guest_calls::{
        Handle,
        events::{EventData, EventResult},
    },
    local_resources::Registry,
};
use maestro_extensions::{
    AfterProviderResponseEvent, AgentEndEvent, AgentStartEvent, BashOperations, BashResult,
    BeforeAgentStartEvent, BeforeAgentStartEventResult, BeforeProviderRequestEvent,
    BeforeProviderRequestEventResult, Cancellation, ContextEvent, ContextEventResult, ExtensionAPI,
    InputEvent, InputEventResult, Local, MessageEndEvent, MessageEndEventResult, MessageStartEvent,
    MessageUpdateEvent, ModelSelectEvent, ResourcesDiscoverEvent, ResourcesDiscoverResult,
    SessionBeforeCompactEvent, SessionBeforeCompactResult, SessionBeforeForkEvent,
    SessionBeforeForkResult, SessionBeforeSwitchEvent, SessionBeforeSwitchResult,
    SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionCompactEvent, SessionShutdownEvent,
    SessionStartEvent, SessionTreeEvent, ThinkingLevelSelectEvent, ToolCallEvent,
    ToolCallEventResult, ToolExecutionEndEvent, ToolExecutionStartEvent, ToolExecutionUpdateEvent,
    ToolResultEvent, ToolResultEventResult, TreePreparation, TurnEndEvent, TurnStartEvent,
    UserBashEvent, UserBashEventResult,
};
use serde::{Deserialize, Serialize};

/// Actual Local-family operations projection retained in the native shell verdict.
type Operations = std::rc::Rc<dyn BashOperations<Local>>;
/// Actual native shell result, not the guest author's optional-field representation.
type ShellResult = UserBashEventResult<Operations, BashResult>;

/// A concrete registration selected by the shared inventory.
trait Binding {
    /// Actual native event record, including its original resources.
    type Event: EventData;
    /// Actual native result record.
    type Result: EventResult;
}
/// Builds only event identity and typed registration dispatch from the shared table.
macro_rules! inventory {
    (before { $($(#[$bm:meta])* $bn:ident($bt:ty),)* }
     session { $($(#[$sm:meta])* $sn:ident / $sd:ident($st:ty, $sdt:ty),)* }
     after { $($(#[$am:meta])* $an:ident($at:ty),)* }) => {
        /// Canonical recognized names; unknown names remain opaque native registrations.
        #[derive(Deserialize)]
        #[serde(rename_all = "snake_case")]
        enum Kind { $($(#[$bm])* $bn,)* $($(#[$sm])* $sd,)* $($(#[$am])* $an,)* }
        /// Type-level associations, not callable or payload substitutes.
        mod bindings {
            $($(#[$bm])* pub(super) struct $bn;)*
            $($(#[$sm])* pub(super) struct $sd;)*
            $($(#[$am])* pub(super) struct $an;)*
        }
        /// Appends known concrete callbacks and retains unknown names without dispatch.
        pub(crate) fn register(api: &ExtensionAPI<Local>, name: String, handler: Handle, registry: Registry) -> Result<(), String> {
            let kind = Kind::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(&name));
            match kind {
                $(Ok(Kind::$bn) => bind::<bindings::$bn>(api, name, &handler, registry),)*
                $(Ok(Kind::$sd) => bind::<bindings::$sd>(api, name, &handler, registry),)*
                $(Ok(Kind::$an) => bind::<bindings::$an>(api, name, &handler, registry),)*
                Err(_) => api.on_retained(name, Box::new(handler)).map_err(|error| error.to_string()),
            }
        }
    };
}
maestro_extensions::extension_event_table!(inventory);

/// Attaches the callback to the original native owner with its exact event/result pair.
fn bind<B: Binding>(
    api: &ExtensionAPI<Local>,
    name: String,
    handler: &Handle,
    registry: Registry,
) -> Result<(), String> {
    let callback = handler.event::<B::Event, B::Result>(name.clone(), registry);
    api.on(name, callback).map_err(|error| error.to_string())
}
/// Defines each inventory entry's native payload and verdict specialization.
macro_rules! binding {
    ($($kind:ident => $event:ty, $result:ty;)*) => {$(
        impl Binding for bindings::$kind { type Event = $event; type Result = $result; }
    )*};
}
binding! {
    ToolCall => ToolCallEvent, ToolCallEventResult;
    ToolResult => ToolResultEvent, ToolResultEventResult;
    UserBash => UserBashEvent, ShellResult;
    ModelSelect => ModelSelectEvent, ();
    ThinkingLevelSelect => ThinkingLevelSelectEvent, ();
    ToolExecutionStart => ToolExecutionStartEvent, ();
    ToolExecutionUpdate => ToolExecutionUpdateEvent, ();
    ToolExecutionEnd => ToolExecutionEndEvent, ();
    Context => ContextEvent, ContextEventResult;
    BeforeAgentStart => BeforeAgentStartEvent, BeforeAgentStartEventResult;
    AgentStart => AgentStartEvent, ();
    AgentEnd => AgentEndEvent, ();
    TurnStart => TurnStartEvent, ();
    TurnEnd => TurnEndEvent, ();
    MessageStart => MessageStartEvent, ();
    MessageUpdate => MessageUpdateEvent, ();
    MessageEnd => MessageEndEvent, MessageEndEventResult;
    ResourcesDiscover => ResourcesDiscoverEvent, ResourcesDiscoverResult;
    SessionStart => SessionStartEvent, ();
    SessionBeforeSwitch => SessionBeforeSwitchEvent, SessionBeforeSwitchResult;
    SessionBeforeFork => SessionBeforeForkEvent, SessionBeforeForkResult;
    SessionBeforeCompact => SessionBeforeCompactEvent, SessionBeforeCompactResult;
    SessionCompact => SessionCompactEvent, ();
    SessionBeforeTree => SessionBeforeTreeEvent, SessionBeforeTreeResult;
    SessionTree => SessionTreeEvent, ();
    SessionShutdown => SessionShutdownEvent, ();
    BeforeProviderRequest => BeforeProviderRequestEvent, BeforeProviderRequestEventResult;
    AfterProviderResponse => AfterProviderResponseEvent, ();
    Input => InputEvent, InputEventResult;
}

/// Encodes a payload with the canonical tag without adding resources to JSON.
fn tagged(value: &impl Serialize, name: &str) -> Result<String, String> {
    let mut data = serde_json::to_value(value).map_err(|error| error.to_string())?;
    let object = data
        .as_object_mut()
        .ok_or_else(|| "event is not an object".to_owned())?;
    object.insert("type".into(), name.into());
    serde_json::to_string(&data).map_err(|error| error.to_string())
}
/// Ordinary records retain the native type's own Serde contract.
macro_rules! records {
    ($($event:ty),* $(,)?) => {$(
        impl EventData for $event {
            fn encode(&self, name: &str) -> Result<String, String> { tagged(self, name) }
            fn apply(&mut self, data: &str) -> Result<(), String> {
                *self = crate::writer_reads::decode(data)?;
                Ok(())
            }
        }
    )*};
}
records! {
    ToolCallEvent,
    ToolResultEvent,
    UserBashEvent,
    ModelSelectEvent,
    ThinkingLevelSelectEvent,
    ToolExecutionStartEvent,
    ToolExecutionUpdateEvent,
    ToolExecutionEndEvent,
    ContextEvent,
    BeforeAgentStartEvent,
    AgentStartEvent,
    AgentEndEvent,
    TurnStartEvent,
    TurnEndEvent,
    MessageStartEvent,
    MessageUpdateEvent,
    MessageEndEvent,
    ResourcesDiscoverEvent,
    SessionStartEvent,
    SessionBeforeSwitchEvent,
    SessionBeforeForkEvent,
    SessionCompactEvent,
    SessionTreeEvent,
    SessionShutdownEvent,
    BeforeProviderRequestEvent,
    InputEvent,
}
impl EventData for AfterProviderResponseEvent {
    fn encode(&self, name: &str) -> Result<String, String> {
        let headers: Vec<_> = self.headers.iter().collect();
        tagged(
            &AfterProviderResponseEvent {
                status: self.status,
                headers,
            },
            name,
        )
    }
    fn apply(&mut self, data: &str) -> Result<(), String> {
        let data =
            crate::writer_reads::decode::<AfterProviderResponseEvent<Vec<(String, String)>>>(data)?;
        self.status = data.status;
        self.headers = data.headers.into_iter().collect();
        Ok(())
    }
}
impl EventData for SessionBeforeCompactEvent {
    fn encode(&self, name: &str) -> Result<String, String> {
        tagged(&self.data, name)
    }
    fn apply(&mut self, data: &str) -> Result<(), String> {
        self.data = crate::writer_reads::decode(data)?;
        Ok(())
    }
    fn signal(&self) -> Option<Cancellation> {
        Some(self.signal.clone())
    }
}
impl EventData for SessionBeforeTreeEvent {
    fn encode(&self, name: &str) -> Result<String, String> {
        tagged(&serde_json::json!({"preparation": self.preparation}), name)
    }
    fn apply(&mut self, data: &str) -> Result<(), String> {
        /// The signal remains the caller's original resource.
        #[derive(Deserialize)]
        struct Data {
            /// Edited tree preparation only.
            preparation: TreePreparation,
        }
        self.preparation = crate::writer_reads::decode::<Data>(data)?.preparation;
        Ok(())
    }
    fn signal(&self) -> Option<Cancellation> {
        Some(self.signal.clone())
    }
}
/// Results without returned operations use their owning record decoders.
macro_rules! results { ($($result:ty),* $(,)?) => {$(
    impl EventResult for $result {
        fn decode(data: &str) -> Result<Self, String> { crate::writer_reads::decode(data) }
    }
)*}; }
results! {
    ToolCallEventResult,
    ToolResultEventResult,
    ContextEventResult,
    BeforeAgentStartEventResult,
    MessageEndEventResult,
    ResourcesDiscoverResult,
    SessionBeforeSwitchResult,
    SessionBeforeForkResult,
    SessionBeforeCompactResult,
    SessionBeforeTreeResult,
    BeforeProviderRequestEventResult,
    InputEventResult,
}
impl EventResult for () {
    fn decode(data: &str) -> Result<Self, String> {
        let _: serde_json::Value = crate::writer_reads::decode(data)?;
        Ok(())
    }
}
impl EventResult for ShellResult {
    fn decode(data: &str) -> Result<Self, String> {
        crate::writer_reads::decode(data)
    }

    fn attach(&mut self, operations: Option<crate::guest_calls::bash::ReturnedBash>) {
        self.operations = operations.map(|operations| std::rc::Rc::new(operations) as Operations);
    }
}

#[cfg(test)]
mod tests;
