//! Typed response chunks: a field of the wrong type reads as absent, like a missing one.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::value::RawValue;

use crate::providers::json_text::{object_record, raw_number};

/// Read a value of the wrong type as absent.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(T::deserialize(<&RawValue>::deserialize(deserializer)?).ok())
}

/// Read a number as the double it rounds to; anything else as absent.
fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(raw_number(<&RawValue>::deserialize(deserializer)?))
}

/// Read a number as a tool-call position; anything else as absent.
fn index<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<StreamIndex>, D::Error> {
    Ok(number(deserializer)?.map(StreamIndex::of))
}

/// Read an object as a record; anything else, arrays included, as absent.
fn record<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(object_record(<&RawValue>::deserialize(deserializer)?))
}

/// Read an array element by element, keeping each position; non-objects become `None`, and a
/// value that is not an array has no elements.
fn records<'de, D, T>(deserializer: D) -> Result<Vec<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let items: Vec<&RawValue> =
        serde_json::from_str(<&RawValue>::deserialize(deserializer)?.get()).unwrap_or_default();
    Ok(items.into_iter().map(object_record).collect())
}

/// One decoded response chunk.
#[derive(Default, Deserialize)]
pub(super) struct Chunk {
    /// Response identifier.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) id: Option<String>,
    /// Model that produced the response.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) model: Option<String>,
    /// Usage reported for the response so far.
    #[serde(default, deserialize_with = "record")]
    pub(super) usage: Option<RawUsage>,
    /// Alternatives; only the first is used.
    #[serde(default, deserialize_with = "records")]
    pub(super) choices: Vec<Option<Choice>>,
    /// The provider's error report, kept as written.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) error: Option<Box<RawValue>>,
}

/// One alternative of a chunk.
#[derive(Default, Deserialize)]
pub(super) struct Choice {
    /// Why generation stopped, once it has.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) finish_reason: Option<String>,
    /// Usage reported by providers that put it here.
    #[serde(default, deserialize_with = "record")]
    pub(super) usage: Option<RawUsage>,
    /// The new content.
    #[serde(default, deserialize_with = "record")]
    pub(super) delta: Option<Delta>,
}

/// New content of a choice.
#[derive(Default, Deserialize)]
pub(super) struct Delta {
    /// Visible text.
    #[serde(default, deserialize_with = "lenient")]
    content: Option<String>,
    /// Reasoning text under the name some servers use.
    #[serde(default, deserialize_with = "lenient")]
    reasoning_content: Option<String>,
    /// Reasoning text under the name other servers use.
    #[serde(default, deserialize_with = "lenient")]
    reasoning: Option<String>,
    /// Reasoning text under a third name.
    #[serde(default, deserialize_with = "lenient")]
    reasoning_text: Option<String>,
    /// Tool-call fragments.
    #[serde(default, deserialize_with = "records")]
    pub(super) tool_calls: Vec<Option<ToolCallDelta>>,
    /// Encrypted reasoning details, kept as received.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) reasoning_details: Option<Vec<Box<RawValue>>>,
}

impl Delta {
    /// The visible text, if any.
    pub(super) fn text(&self) -> Option<&str> {
        self.content.as_deref().filter(|text| !text.is_empty())
    }

    /// The first nonempty reasoning text and the field that carried it.
    pub(super) fn reasoning_text(&self) -> Option<(&'static str, &str)> {
        [
            ("reasoning_content", &self.reasoning_content),
            ("reasoning", &self.reasoning),
            ("reasoning_text", &self.reasoning_text),
        ]
        .into_iter()
        .find_map(|(field, text)| {
            text.as_deref()
                .filter(|text| !text.is_empty())
                .map(|text| (field, text))
        })
    }
}

/// Position of a tool call within the response, compared as the number it spells: `0`, `0.0`,
/// `0e0` and `-0` are one position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct StreamIndex(u64);

impl StreamIndex {
    /// The position a number names; both zeros share one.
    fn of(number: f64) -> Self {
        Self(if number == 0.0 { 0 } else { number.to_bits() })
    }
}

/// A fragment of a tool call.
#[derive(Default, Deserialize)]
pub(super) struct ToolCallDelta {
    /// Position of the call within the response.
    #[serde(default, deserialize_with = "index")]
    pub(super) index: Option<StreamIndex>,
    /// Call identifier.
    #[serde(default, deserialize_with = "lenient")]
    id: Option<String>,
    /// Function name and argument text.
    #[serde(default, deserialize_with = "record")]
    function: Option<FunctionDelta>,
}

impl ToolCallDelta {
    /// The identifier, if nonempty.
    pub(super) fn id(&self) -> Option<&str> {
        self.id.as_deref().filter(|id| !id.is_empty())
    }

    /// The function name, if nonempty.
    pub(super) fn name(&self) -> Option<&str> {
        let name = self.function.as_ref()?.name.as_deref();
        name.filter(|name| !name.is_empty())
    }

    /// The argument text, if nonempty.
    pub(super) fn arguments(&self) -> Option<&str> {
        let arguments = self.function.as_ref()?.arguments.as_deref();
        arguments.filter(|arguments| !arguments.is_empty())
    }
}

/// The function part of a tool-call fragment.
#[derive(Default, Deserialize)]
struct FunctionDelta {
    /// Function name.
    #[serde(default, deserialize_with = "lenient")]
    name: Option<String>,
    /// Argument text.
    #[serde(default, deserialize_with = "lenient")]
    arguments: Option<String>,
}

/// Token counts as a provider reports them.
#[derive(Default, Deserialize)]
pub(super) struct RawUsage {
    /// Tokens in the prompt, cache hits included.
    #[serde(default, deserialize_with = "number")]
    pub(super) prompt_tokens: Option<f64>,
    /// Tokens generated, reasoning included.
    #[serde(default, deserialize_with = "number")]
    pub(super) completion_tokens: Option<f64>,
    /// Cache hits reported at the top level by some providers.
    #[serde(default, deserialize_with = "number")]
    pub(super) prompt_cache_hit_tokens: Option<f64>,
    /// Cache counts reported under the prompt details.
    #[serde(default, deserialize_with = "record")]
    pub(super) prompt_tokens_details: Option<PromptDetails>,
}

/// Cache counts within the prompt tokens.
#[derive(Default, Deserialize)]
pub(super) struct PromptDetails {
    /// Tokens served from the cache.
    #[serde(default, deserialize_with = "number")]
    pub(super) cached_tokens: Option<f64>,
    /// Tokens written to the cache.
    #[serde(default, deserialize_with = "number")]
    pub(super) cache_write_tokens: Option<f64>,
}
