//! Generic extension event data.
use super::object;
use crate::{presence::Presence, types::ImageContent};
use serde::{Deserialize, Deserializer, Serialize, de::IntoDeserializer};
use std::ops::{Deref, DerefMut};
/// Reads a string, then deserializes `T` from it.
///
/// # Errors
/// Returns an error if string reading or `T` deserialization fails.
fn literal<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<T, D::Error> {
    T::deserialize(String::deserialize(deserializer)?.into_deserializer())
}

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
    #[serde(deserialize_with = "literal")]
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
    #[serde(deserialize_with = "literal")]
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
    #[serde(deserialize_with = "literal")]
    pub reason: SessionBeforeSwitchReason,
    /// The session file switched to, when the host supplies it.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub target_session_file: Presence<String>,
}

/// The session is about to fork.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "F: Deserialize<'de>"))]
pub struct SessionBeforeForkEvent<F> {
    /// The entry the fork starts from.
    pub entry_id: String,
    /// Where the fork starts relative to the entry.
    #[serde(deserialize_with = "literal")]
    pub position: F,
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
    #[serde(deserialize_with = "literal")]
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
pub struct BeforeProviderRequestEvent<P> {
    /// The request body in the caller's opaque payload representation.
    pub payload: P,
}

/// A replacement request body in the caller's payload representation.
/// A supplied null payload differs from returning no result.
pub type BeforeProviderRequestEventResult<P> = P;

/// A provider answered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AfterProviderResponseEvent<H> {
    /// The response status is an editable number, not a validated HTTP status.
    pub status: f64,
    /// Uninterpreted header names and values in the supplied collection.
    /// Native maps order by name; guest pairs retain host order.
    pub headers: H,
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
    #[serde(
        default,
        skip_serializing_if = "Presence::is_missing",
        deserialize_with = "object::records"
    )]
    pub images: Presence<Vec<ImageContent>>,
    /// Where the input came from.
    #[serde(deserialize_with = "literal")]
    pub source: InputSource,
    /// How the input is queued when it arrives while a response streams; absent otherwise.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub streaming_behavior: Presence<crate::messages::UserMessageDelivery>,
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
#[serde(bound(deserialize = "P: Deserialize<'de>, E: Deserialize<'de>"))]
pub struct SessionBeforeCompactEventData<P, E> {
    /// What the compaction is about to summarize.
    #[serde(deserialize_with = "object::record")]
    pub preparation: P,
    /// Ordered branch entries supplied independently of summary messages.
    pub branch_entries: Vec<E>,
    /// Instructions the user gave the compaction.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub custom_instructions: Presence<String>,
}

/// A compaction is about to happen: its data plus the cancellation signal the handler may
/// retain.
#[derive(Debug)]
pub struct SessionBeforeCompactEvent<P, E, S> {
    /// The serializable data.
    pub data: SessionBeforeCompactEventData<P, E>,
    /// Reports whether the host cancelled the compaction.
    pub signal: S,
}

impl<P, E, S> Deref for SessionBeforeCompactEvent<P, E, S> {
    type Target = SessionBeforeCompactEventData<P, E>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<P, E, S> DerefMut for SessionBeforeCompactEvent<P, E, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

/// Decision of a compaction handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound(deserialize = "C: Deserialize<'de>"))]
pub struct SessionBeforeCompactResult<C> {
    /// Whether the compaction is cancelled.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cancel: Presence<bool>,
    /// A replacement compaction.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub compaction: Presence<C>,
}
