//! Typed incoming events: the fields a reduction reads, over the raw JSON members of each event.

use serde::Deserialize;
use serde_json::value::RawValue;

use crate::providers::json_text::{lenient, number, required, try_object_record};

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

/// The only member of an event root that selects its record.
#[derive(Deserialize)]
struct Root {
    /// The event type.
    #[serde(default, deserialize_with = "lenient")]
    r#type: Option<String>,
}

/// Read the type of an event from its root; an object without a text `type` has none.
///
/// # Errors
/// Fails with the reader's cause for a `null` root, which cannot carry an event. Every other
/// root that is not an object has no type.
pub(super) fn event_type(raw: &RawValue) -> Result<Option<String>, serde_json::Error> {
    match try_object_record::<Root>(raw) {
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
    /// Usage so far.
    #[serde(deserialize_with = "required")]
    pub(super) usage: Usage,
}

/// The opening of a content block.
#[derive(Deserialize)]
pub(super) struct ContentBlockStart {
    /// Position of the block on the wire.
    #[serde(default, deserialize_with = "number")]
    pub(super) index: Option<f64>,
    /// The block as it opens.
    #[serde(deserialize_with = "required")]
    pub(super) content_block: OpenedBlock,
}

/// A content block as it opens.
#[derive(Deserialize)]
pub(super) struct OpenedBlock {
    /// Kind of block.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) r#type: Option<String>,
    /// Opaque payload of a redacted reasoning block.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) data: Option<String>,
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
    #[serde(default, deserialize_with = "number")]
    pub(super) index: Option<f64>,
    /// The change.
    #[serde(deserialize_with = "required")]
    pub(super) delta: BlockDelta,
}

/// One change to a content block.
#[derive(Deserialize)]
pub(super) struct BlockDelta {
    /// Kind of change.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) r#type: Option<String>,
    /// Text to append.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) text: Option<String>,
    /// Reasoning to append.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) thinking: Option<String>,
    /// Argument text to append.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) partial_json: Option<String>,
    /// Signature fragment to append.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) signature: Option<String>,
}

/// The close of a content block.
#[derive(Deserialize)]
pub(super) struct ContentBlockStop {
    /// Position of the block on the wire.
    #[serde(default, deserialize_with = "number")]
    pub(super) index: Option<f64>,
}

/// Message-level changes.
#[derive(Deserialize)]
pub(super) struct MessageDelta {
    /// The outcome, once known.
    #[serde(deserialize_with = "required")]
    pub(super) delta: MessageOutcome,
    /// Usage so far.
    #[serde(deserialize_with = "required")]
    pub(super) usage: Usage,
}

/// How the message ended.
#[derive(Deserialize)]
pub(super) struct MessageOutcome {
    /// Why generation stopped.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) stop_reason: Option<String>,
}
