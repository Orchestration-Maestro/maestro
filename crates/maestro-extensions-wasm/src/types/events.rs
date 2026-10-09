//! Author-facing events and results, and the JSON data that carries them.
//!
//! Rust records define the payloads once. An event crosses the component boundary as one JSON
//! document tagged with its kind: the flat `EventData` holds the serializable payload of each
//! kind, and the owned host resources an event may carry travel beside it and are attached to
//! the author's event. Optional properties are [`Presence`] values.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use super::context::{AbortSignal, ExtensionContext};
use super::extension_result::ExtensionFuture;
use super::number;
use super::presence::Presence;
use crate::compaction::{CompactionPreparation, CompactionResult};
use crate::models::ImageContent;

/// Why resources are being discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourcesDiscoverReason {
    /// The session is starting.
    Startup,
    /// The session reloads its resources.
    Reload,
}

/// Resources are being discovered for a working directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesDiscoverEvent {
    /// The working directory, carried as given.
    pub cwd: String,
    /// What triggered the discovery.
    pub reason: ResourcesDiscoverReason,
}

/// Resource paths an extension contributes; each list is independently optional and keeps its
/// order and duplicates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesDiscoverResult {
    /// Skill paths.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub skill_paths: Presence<Vec<String>>,
    /// Prompt paths.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub prompt_paths: Presence<Vec<String>>,
    /// Theme paths.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub theme_paths: Presence<Vec<String>>,
}

/// Why a session started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStartReason {
    /// The process started.
    Startup,
    /// The session reloaded.
    Reload,
    /// A new session replaced the previous one.
    New,
    /// An existing session was resumed.
    Resume,
    /// The session was forked.
    Fork,
}

/// A session has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartEvent {
    /// Why the session started.
    pub reason: SessionStartReason,
    /// The session file that was active before, when the host supplies it.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub previous_session_file: Presence<String>,
}

/// Why the session is about to switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionBeforeSwitchReason {
    /// A new session is about to start.
    New,
    /// An existing session is about to be resumed.
    Resume,
}

/// The session is about to switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeSwitchEvent {
    /// Why the session switches.
    pub reason: SessionBeforeSwitchReason,
    /// The session file switched to, when the host supplies it.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub target_session_file: Presence<String>,
}

/// Where a fork starts relative to its entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForkPosition {
    /// Before the entry.
    Before,
    /// At the entry.
    At,
}

/// The session is about to fork.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeForkEvent {
    /// The entry the fork starts from.
    pub entry_id: String,
    /// Where the fork starts relative to the entry.
    pub position: ForkPosition,
}

/// Why a runtime shuts down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionShutdownReason {
    /// The process quits.
    Quit,
    /// The session reloads.
    Reload,
    /// A new session replaces this one.
    New,
    /// Another session is resumed.
    Resume,
    /// The session is forked.
    Fork,
}

/// A runtime is shutting down.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionShutdownEvent {
    /// Why the runtime shuts down.
    pub reason: SessionShutdownReason,
    /// The session file that follows, when the host supplies it.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub target_session_file: Presence<String>,
}

/// Decision of a handler for a session switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBeforeSwitchResult {
    /// Whether the switch is cancelled.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
}

/// Decision of a handler for a fork; the two decisions are independent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeForkResult {
    /// Whether the fork is cancelled.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
    /// Whether the conversation is left as it was.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub skip_conversation_restore: Presence<bool>,
}

/// A provider request is about to be sent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeProviderRequestEvent {
    /// The request body as opaque JSON text.
    pub payload: String,
}

/// A replacement request body as opaque JSON text; the text `null` replaces the payload with
/// null and differs from returning no result.
pub type BeforeProviderRequestEventResult = String;

/// A provider answered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AfterProviderResponseEvent {
    /// The response status. It is an editable number, not a validated HTTP status; a
    /// nonfinite value is carried by its bits.
    #[serde(with = "number")]
    pub status: f64,
    /// Header names and values in the order the host listed them, uninterpreted.
    pub headers: Vec<(String, String)>,
}

/// Where an input came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputSource {
    /// Typed by the user.
    Interactive,
    /// Sent through the RPC interface.
    Rpc,
    /// Sent by an extension.
    Extension,
}

/// A user input before agent processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    /// The input text.
    pub text: String,
    /// The attached images.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub images: Presence<Vec<ImageContent>>,
    /// Where the input came from.
    pub source: InputSource,
}

/// Replacement text and images of an input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputTransform {
    /// The replacement text.
    pub text: String,
    /// The replacement images.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub images: Presence<Vec<ImageContent>>,
}

/// What an input handler decided.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum InputEventResult {
    /// Leave the input as it is.
    Continue,
    /// Replace the input.
    Transform(InputTransform),
    /// The extension handled the input; nothing more runs.
    Handled,
}

/// The data of a compaction that is about to happen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBeforeCompactEventData {
    /// What the compaction is about to summarize.
    pub preparation: CompactionPreparation,
    /// Instructions the user gave the compaction.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub custom_instructions: Presence<String>,
}

/// A compaction is about to happen: its data plus the cancellation signal the handler may
/// retain.
#[derive(Debug)]
pub struct SessionBeforeCompactEvent {
    /// The serializable data.
    pub data: SessionBeforeCompactEventData,
    /// Reports whether the host cancelled the compaction.
    pub signal: AbortSignal,
}

impl Deref for SessionBeforeCompactEvent {
    type Target = SessionBeforeCompactEventData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for SessionBeforeCompactEvent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// Decision of a compaction handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBeforeCompactResult {
    /// Whether the compaction is cancelled.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
    /// A replacement compaction.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub compaction: Presence<CompactionResult>,
}

/// Session events.
#[derive(Debug)]
pub enum SessionEvent {
    /// A session started.
    Start(SessionStartEvent),
    /// The session is about to switch.
    BeforeSwitch(SessionBeforeSwitchEvent),
    /// The session is about to fork.
    BeforeFork(SessionBeforeForkEvent),
    /// A compaction is about to happen.
    BeforeCompact(SessionBeforeCompactEvent),
    /// A runtime is shutting down.
    Shutdown(SessionShutdownEvent),
}

/// An event delivered to handlers, in the shape authors write against.
#[derive(Debug)]
pub enum ExtensionEvent {
    /// Resources are being discovered.
    ResourcesDiscover(ResourcesDiscoverEvent),
    /// A session event.
    Session(SessionEvent),
    /// A provider request is about to be sent.
    BeforeProviderRequest(BeforeProviderRequestEvent),
    /// A provider answered.
    AfterProviderResponse(AfterProviderResponseEvent),
    /// A user input before agent processing.
    Input(InputEvent),
}

/// A handler's verdict or replacement. The invoking event identifies which contract applies;
/// the variants carry no tag on the wire.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ExtensionEventResult {
    /// Resource paths to add.
    ResourcesDiscover(ResourcesDiscoverResult),
    /// Whether to cancel a switch.
    SessionBeforeSwitch(SessionBeforeSwitchResult),
    /// Whether to cancel a fork or keep the conversation.
    SessionBeforeFork(SessionBeforeForkResult),
    /// Verdict or replacement of a compaction handler.
    SessionBeforeCompact(SessionBeforeCompactResult),
    /// A replacement request body.
    BeforeProviderRequest(BeforeProviderRequestEventResult),
    /// What an input handler decided.
    Input(InputEventResult),
}

/// A registered event handler. It receives the event mutably and answers with an optional
/// result or a failure; the adapter returns the event as the handler left it, whether the
/// handler answered or failed, unless it replaced the event with one of another kind.
pub type ExtensionHandler = Rc<
    dyn for<'a> Fn(
        &'a mut ExtensionEvent,
        ExtensionContext,
    ) -> ExtensionFuture<'a, Option<ExtensionEventResult>>,
>;
