//! Typed incoming events: the fields a reduction reads, over the raw JSON members of each event.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

use crate::providers::json_text::{
    EntryKey, lenient, member, number, raw_number, required, try_object_record,
};

/// A decoded event of a message stream.
pub(super) enum Event {
    /// The message begins.
    MessageStart(MessageStart),
    /// A content block opens.
    ContentBlockStart(ContentBlockStart),
    /// A content block grows.
    ContentBlockDelta(ContentBlockDelta),
    /// A content block closes.
    ContentBlockStop(ContentBlockStop),
    /// The message-level fields change.
    MessageDelta(MessageDelta),
    /// The message is complete.
    MessageStop,
}

/// The only member of a record that selects its kind.
#[derive(Deserialize)]
struct Tagged {
    /// The kind.
    #[serde(default, deserialize_with = "lenient")]
    r#type: Option<String>,
}

/// Read the type of an event from its root; an object without a text `type` has none.
///
/// # Errors
/// Fails with the reader's cause for a `null` root, which cannot carry an event. Every other
/// root that is not an object has no type.
pub(super) fn event_type(raw: &RawValue) -> Result<Option<String>, serde_json::Error> {
    match try_object_record::<Tagged>(raw) {
        Ok(root) => Ok(root.r#type),
        Err(error) if raw.get() == "null" => Err(error),
        Err(_) => Ok(None),
    }
}

/// Decode the record of an event of a known type; other types are `None`.
///
/// # Errors
/// Fails with the reader's cause when a member the event requires is missing or not an object.
pub(super) fn decode(event_type: &str, raw: &RawValue) -> Result<Option<Event>, serde_json::Error> {
    Ok(Some(match event_type {
        "message_start" => Event::MessageStart(try_object_record(raw)?),
        "content_block_start" => Event::ContentBlockStart(try_object_record(raw)?),
        "content_block_delta" => Event::ContentBlockDelta(try_object_record(raw)?),
        "content_block_stop" => Event::ContentBlockStop(try_object_record(raw)?),
        "message_delta" => Event::MessageDelta(try_object_record(raw)?),
        "message_stop" => Event::MessageStop,
        _ => return Ok(None),
    }))
}

/// Token counts as the service reports them; a count that is missing or null is absent.
#[derive(Deserialize, Default)]
pub(super) struct Usage {
    /// Prompt tokens.
    #[serde(default, rename = "input_tokens", deserialize_with = "number")]
    pub(super) input: Option<f64>,
    /// Generated tokens.
    #[serde(default, rename = "output_tokens", deserialize_with = "number")]
    pub(super) output: Option<f64>,
    /// Prompt tokens read from the cache.
    #[serde(
        default,
        rename = "cache_read_input_tokens",
        deserialize_with = "number"
    )]
    pub(super) cache_read: Option<f64>,
    /// Prompt tokens written to the cache.
    #[serde(
        default,
        rename = "cache_creation_input_tokens",
        deserialize_with = "number"
    )]
    pub(super) cache_write: Option<f64>,
}

impl Usage {
    /// Decode the usage an event carries when its reduction reads it.
    ///
    /// # Errors
    /// Fails with the reader's cause when the member is missing or is not an object.
    pub(super) fn read(raw: Option<&RawValue>) -> Result<Self, serde_json::Error> {
        raw.map_or_else(
            || Err(serde_json::Error::missing_field("usage")),
            try_object_record,
        )
    }
}

/// The beginning of a message.
#[derive(Deserialize)]
pub(super) struct MessageStart {
    /// The message and its initial usage.
    #[serde(deserialize_with = "required")]
    pub(super) message: StartedMessage,
}

/// The message a start event announces.
#[derive(Deserialize)]
pub(super) struct StartedMessage {
    /// Response identifier.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) id: Option<String>,
    /// Usage so far, as written.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) usage: Option<Box<RawValue>>,
}

/// Where a block sits on the wire: the `index` member of its events, compared as strict
/// equality compares the values.
#[derive(Default, PartialEq)]
pub(super) enum Position {
    /// The member is absent.
    #[default]
    Missing,
    /// The member is `null`.
    Null,
    /// A number, as the double it rounds to.
    Number(f64),
    /// Decoded text, retaining unpaired UTF-16 units.
    Text(EntryKey),
    /// A boolean.
    Flag(bool),
    /// An array or an object.
    Distinct,
}

impl Position {
    /// Whether `other` names the same block: a distinct position names none, not even itself.
    pub(super) fn names(&self, other: &Self) -> bool {
        *self != Self::Distinct && self == other
    }
}

impl<'de> Deserialize<'de> for Position {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?;
        Ok(match (raw_number(raw), raw.get().as_bytes().first()) {
            (Some(number), _) => Self::Number(number),
            (_, Some(b'n')) => Self::Null,
            (_, Some(b't')) => Self::Flag(true),
            (_, Some(b'f')) => Self::Flag(false),
            (_, Some(b'"')) => serde_json::from_str(raw.get()).map_or(Self::Distinct, Self::Text),
            _ => Self::Distinct,
        })
    }
}

/// Read the kind of a record that must be an object, and keep the record to read the members
/// that kind uses.
fn tagged<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<(Option<String>, &'de RawValue), D::Error> {
    let raw = <&RawValue>::deserialize(deserializer)?;
    let kind = try_object_record::<Tagged>(raw)
        .map_err(D::Error::custom)?
        .r#type;
    Ok((kind, raw))
}

/// A member read as text; absent when it is missing or is not text.
fn text_member(raw: &RawValue, name: &str) -> Option<String> {
    serde_json::from_str(member(raw, name)?.get()).ok()
}

/// The opening of a content block.
#[derive(Deserialize)]
pub(super) struct ContentBlockStart {
    /// Position of the block on the wire.
    #[serde(default)]
    pub(super) index: Position,
    /// The block as it opens.
    pub(super) content_block: OpenedBlock,
}

/// A content block as it opens.
pub(super) enum OpenedBlock {
    /// A text block.
    Text,
    /// A reasoning block.
    Thinking,
    /// A redacted reasoning block with its opaque payload.
    Redacted(Option<String>),
    /// A tool call.
    ToolUse(ToolUse),
    /// A kind the message has no block for.
    Other,
}

impl<'de> Deserialize<'de> for OpenedBlock {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (kind, raw) = tagged(deserializer)?;
        Ok(match kind.as_deref() {
            Some("text") => Self::Text,
            Some("thinking") => Self::Thinking,
            Some("redacted_thinking") => Self::Redacted(text_member(raw, "data")),
            Some("tool_use") => Self::ToolUse(try_object_record(raw).map_err(D::Error::custom)?),
            _ => Self::Other,
        })
    }
}

/// The members of a tool call as it opens.
#[derive(Deserialize)]
pub(super) struct ToolUse {
    /// Tool call identifier.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) id: Option<String>,
    /// Tool name.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) name: Option<String>,
    /// Arguments the tool call opens with, as written.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) input: Option<Box<RawValue>>,
}

/// New content of a block.
#[derive(Deserialize)]
pub(super) struct ContentBlockDelta {
    /// Position of the block on the wire.
    #[serde(default)]
    pub(super) index: Position,
    /// The change.
    pub(super) delta: BlockDelta,
}

/// One change to a content block.
pub(super) enum BlockDelta {
    /// Text to append.
    Text(String),
    /// Reasoning to append.
    Thinking(String),
    /// Signature fragment to append.
    Signature(String),
    /// Argument text to append.
    ArgumentFragment(String),
    /// A change of another kind, or one that lacks the member its kind carries.
    Other,
}

impl<'de> Deserialize<'de> for BlockDelta {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (kind, raw) = tagged(deserializer)?;
        let (variant, name): (fn(String) -> Self, _) = match kind.as_deref() {
            Some("text_delta") => (Self::Text, "text"),
            Some("thinking_delta") => (Self::Thinking, "thinking"),
            Some("signature_delta") => (Self::Signature, "signature"),
            Some("input_json_delta") => (Self::ArgumentFragment, "partial_json"),
            _ => return Ok(Self::Other),
        };
        Ok(text_member(raw, name).map_or(Self::Other, variant))
    }
}

/// The close of a content block.
#[derive(Deserialize)]
pub(super) struct ContentBlockStop {
    /// Position of the block on the wire.
    #[serde(default)]
    pub(super) index: Position,
}

/// Message-level changes.
#[derive(Deserialize)]
pub(super) struct MessageDelta {
    /// The outcome, once known.
    #[serde(deserialize_with = "required")]
    pub(super) delta: MessageOutcome,
    /// Usage so far, as written.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) usage: Option<Box<RawValue>>,
}

/// How the message ended.
#[derive(Deserialize)]
pub(super) struct MessageOutcome {
    /// Why generation stopped.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) stop_reason: Option<String>,
}
