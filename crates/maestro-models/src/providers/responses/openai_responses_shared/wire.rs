//! Typed response events: a field of the wrong type reads as absent, a malformed one fails
//! the event, and members no event kind reads are never decoded.

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, Error as _};
use serde_json::error::Category;
use serde_json::value::RawValue;

use super::native;
use crate::providers::json_text::{object_fields, raw_json, raw_number};
use crate::{DiagnosticErrorInfo, TextPhase};

/// Read a field of the wrong type as absent; any other failure, such as a string holding a
/// lone surrogate, fails the event.
fn field<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    match T::deserialize(<&RawValue>::deserialize(deserializer)?) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.classify() == Category::Data => Ok(None),
        Err(error) => Err(D::Error::custom(error)),
    }
}

/// An error member as the service sent it.
#[derive(Default)]
pub(super) enum Spelled {
    /// The member is missing, or has a type no error member has.
    #[default]
    Missing,
    /// The member is JSON `null`.
    Null,
    /// The member is text.
    Text(String),
}

impl std::fmt::Display for Spelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Missing => "undefined",
            Self::Null => "null",
            Self::Text(text) => text,
        })
    }
}

/// Read an error member as text, `null`, or missing.
fn spelled<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Spelled, D::Error> {
    Ok(match field::<_, Option<String>>(deserializer)? {
        None => Spelled::Missing,
        Some(None) => Spelled::Null,
        Some(Some(text)) => Spelled::Text(text),
    })
}

/// Read a number as the double it rounds to; anything else as absent.
fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(raw_number(<&RawValue>::deserialize(deserializer)?))
}

/// Read an object as a record; anything else, arrays included, as absent.
fn record<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    object_fields(<&RawValue>::deserialize(deserializer)?).map_err(D::Error::custom)
}

/// Read an array element by element, keeping each position; a non-object becomes `None`, and a
/// value that is not an array has no elements.
fn records<'de, D, T>(deserializer: D) -> Result<Vec<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let items: Vec<&RawValue> =
        serde_json::from_str(<&RawValue>::deserialize(deserializer)?.get()).unwrap_or_default();
    items
        .into_iter()
        .map(object_fields)
        .collect::<Result<_, _>>()
        .map_err(D::Error::custom)
}

/// Decode one event; text that is not JSON fails, and one that is not an object is `None`.
///
/// # Errors
/// Returns the parser failure for malformed text, or for a malformed member the event reads.
pub(super) fn event(text: &str) -> Result<Option<Event>, DiagnosticErrorInfo> {
    raw_json(text)
        .and_then(object_fields)
        .map_err(|error| native(&error))
}

/// Decode the item an event carries.
///
/// # Errors
/// Returns the failure for a malformed member the item reads.
pub(super) fn item(raw: &RawValue) -> Result<Option<Item>, DiagnosticErrorInfo> {
    object_fields(raw).map_err(|error| native(&error))
}

/// What a response event announces; kinds the reducer does not know are ignored.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum Kind {
    /// A response began.
    #[serde(rename = "response.created")]
    Created,
    /// An output item opened.
    #[serde(rename = "response.output_item.added")]
    ItemAdded,
    /// An output item is complete.
    #[serde(rename = "response.output_item.done")]
    ItemDone,
    /// A reasoning summary part opened.
    #[serde(rename = "response.reasoning_summary_part.added")]
    SummaryPartAdded,
    /// Text arrived for the open reasoning summary part.
    #[serde(rename = "response.reasoning_summary_text.delta")]
    SummaryTextDelta,
    /// A reasoning summary part is complete.
    #[serde(rename = "response.reasoning_summary_part.done")]
    SummaryPartDone,
    /// Reasoning text arrived.
    #[serde(rename = "response.reasoning_text.delta")]
    ReasoningTextDelta,
    /// A message content part opened.
    #[serde(rename = "response.content_part.added")]
    ContentPartAdded,
    /// Text arrived for the open output text part.
    #[serde(rename = "response.output_text.delta")]
    OutputTextDelta,
    /// Text arrived for the open refusal part.
    #[serde(rename = "response.refusal.delta")]
    RefusalDelta,
    /// Argument text arrived for the open function call.
    #[serde(rename = "response.function_call_arguments.delta")]
    ArgumentsDelta,
    /// The arguments of the open function call are complete.
    #[serde(rename = "response.function_call_arguments.done")]
    ArgumentsDone,
    /// The response finished.
    #[serde(rename = "response.completed")]
    Completed,
    /// The response stopped before finishing.
    #[serde(rename = "response.incomplete")]
    Incomplete,
    /// The response failed.
    #[serde(rename = "response.failed")]
    Failed,
    /// The service reported an error.
    #[serde(rename = "error")]
    Error,
    /// Any other kind.
    #[serde(other)]
    Other,
}

/// One response event, with the members of every kind it can have.
#[derive(Deserialize)]
pub(super) struct Event {
    /// What the event announces.
    #[serde(rename = "type", default, deserialize_with = "field")]
    pub(super) kind: Option<Kind>,
    /// The response the event reports on.
    #[serde(default, deserialize_with = "record")]
    pub(super) response: Option<Response>,
    /// The output item, kept as written until a kind needs it.
    #[serde(default, deserialize_with = "field")]
    pub(super) item: Option<Box<RawValue>>,
    /// The part an added event opens.
    #[serde(default, deserialize_with = "record")]
    pub(super) part: Option<Part>,
    /// The text a delta event adds.
    #[serde(default, deserialize_with = "field")]
    pub(super) delta: Option<String>,
    /// The full argument text a done event reports.
    #[serde(default, deserialize_with = "field")]
    pub(super) arguments: Option<String>,
    /// The error code of an `error` event.
    #[serde(default, deserialize_with = "spelled")]
    pub(super) code: Spelled,
    /// The error message of an `error` event.
    #[serde(default, deserialize_with = "spelled")]
    pub(super) message: Spelled,
}

/// What an output item holds; kinds the reducer does not know are ignored.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum ItemKind {
    /// Reasoning output.
    #[serde(rename = "reasoning")]
    Reasoning,
    /// An assistant message.
    #[serde(rename = "message")]
    Message,
    /// A function call.
    #[serde(rename = "function_call")]
    FunctionCall,
    /// Any other kind.
    #[serde(other)]
    Other,
}

/// An output item, with the members of every kind it can have.
#[derive(Deserialize)]
pub(super) struct Item {
    /// What the item holds.
    #[serde(rename = "type", default, deserialize_with = "field")]
    pub(super) kind: Option<ItemKind>,
    /// Item identifier.
    #[serde(default, deserialize_with = "field")]
    pub(super) id: Option<String>,
    /// Identifier of a function call.
    #[serde(default, deserialize_with = "field")]
    pub(super) call_id: Option<String>,
    /// Name of a function call.
    #[serde(default, deserialize_with = "field")]
    pub(super) name: Option<String>,
    /// Argument text of a function call.
    #[serde(default, deserialize_with = "field")]
    pub(super) arguments: Option<String>,
    /// Phase of a message.
    #[serde(default, deserialize_with = "field")]
    pub(super) phase: Option<TextPhase>,
    /// Summary parts of reasoning.
    #[serde(default, deserialize_with = "records")]
    pub(super) summary: Vec<Option<Part>>,
    /// Content parts of a message, or the reasoning text parts of reasoning.
    #[serde(default, deserialize_with = "records")]
    pub(super) content: Vec<Option<Part>>,
}

/// What a content part holds; kinds other than the two a message reduces are `Other`.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PartKind {
    /// Answer text.
    #[serde(rename = "output_text")]
    OutputText,
    /// A refusal.
    #[serde(rename = "refusal")]
    Refusal,
    /// Any other kind.
    #[serde(other)]
    Other,
}

/// A summary or content part.
#[derive(Deserialize)]
pub(super) struct Part {
    /// What the part holds.
    #[serde(rename = "type", default, deserialize_with = "field")]
    pub(super) kind: Option<PartKind>,
    /// The text of a text or summary part.
    #[serde(default, deserialize_with = "field")]
    pub(super) text: Option<String>,
    /// The text of a refusal.
    #[serde(default, deserialize_with = "field")]
    pub(super) refusal: Option<String>,
}

/// The response an event reports on.
#[derive(Default, Deserialize)]
pub(super) struct Response {
    /// Response identifier.
    #[serde(default, deserialize_with = "field")]
    pub(super) id: Option<String>,
    /// Status of the response.
    #[serde(default, deserialize_with = "field")]
    pub(super) status: Option<String>,
    /// Service tier the response was served at.
    #[serde(default, deserialize_with = "field")]
    pub(super) service_tier: Option<String>,
    /// Token counts.
    #[serde(default, deserialize_with = "record")]
    pub(super) usage: Option<Usage>,
    /// The error that failed the response.
    #[serde(default, deserialize_with = "record")]
    pub(super) error: Option<ErrorDetail>,
    /// Why the response stopped early.
    #[serde(default, deserialize_with = "record")]
    pub(super) incomplete_details: Option<IncompleteDetails>,
}

/// Token counts as the service reports them.
#[derive(Deserialize)]
pub(super) struct Usage {
    /// Tokens in the prompt, cache hits included.
    #[serde(default, deserialize_with = "number")]
    pub(super) input_tokens: Option<f64>,
    /// Tokens generated.
    #[serde(default, deserialize_with = "number")]
    pub(super) output_tokens: Option<f64>,
    /// Tokens in total.
    #[serde(default, deserialize_with = "number")]
    pub(super) total_tokens: Option<f64>,
    /// Cache counts within the prompt tokens.
    #[serde(default, deserialize_with = "record")]
    pub(super) input_tokens_details: Option<InputDetails>,
}

/// Cache counts within the prompt tokens.
#[derive(Deserialize)]
pub(super) struct InputDetails {
    /// Tokens served from the cache.
    #[serde(default, deserialize_with = "number")]
    pub(super) cached_tokens: Option<f64>,
}

/// The error that failed a response.
#[derive(Deserialize)]
pub(super) struct ErrorDetail {
    /// Error code.
    #[serde(default, deserialize_with = "field")]
    pub(super) code: Option<String>,
    /// Error message.
    #[serde(default, deserialize_with = "field")]
    pub(super) message: Option<String>,
}

/// Why a response stopped early.
#[derive(Deserialize)]
pub(super) struct IncompleteDetails {
    /// The stated reason.
    #[serde(default, deserialize_with = "field")]
    pub(super) reason: Option<String>,
}
