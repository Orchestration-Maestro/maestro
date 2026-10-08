//! Typed response chunks: a field of the wrong type reads as absent, like a missing one.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Read a value of the wrong type as absent.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(T::deserialize(Value::deserialize(deserializer)?).ok())
}

/// Read an object as a record; anything else, arrays included, as absent.
fn record<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(object_record(Value::deserialize(deserializer)?))
}

/// Read an array element by element, keeping each position; non-objects become `None`.
fn records<'de, D, T>(deserializer: D) -> Result<Vec<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(items) => items.into_iter().map(object_record).collect(),
        _ => Vec::new(),
    })
}

/// Decode an object into a record.
fn object_record<T: DeserializeOwned>(value: Value) -> Option<T> {
    if value.is_object() {
        T::deserialize(value).ok()
    } else {
        None
    }
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
    pub(super) reasoning_details: Option<Vec<Value>>,
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

/// A fragment of a tool call.
#[derive(Default, Deserialize)]
pub(super) struct ToolCallDelta {
    /// Position of the call within the response.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) index: Option<i64>,
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
    #[serde(default, deserialize_with = "lenient")]
    pub(super) prompt_tokens: Option<f64>,
    /// Tokens generated, reasoning included.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) completion_tokens: Option<f64>,
    /// Cache hits reported at the top level by some providers.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) prompt_cache_hit_tokens: Option<f64>,
    /// Cache counts reported under the prompt details.
    #[serde(default, deserialize_with = "record")]
    pub(super) prompt_tokens_details: Option<PromptDetails>,
}

/// Cache counts within the prompt tokens.
#[derive(Default, Deserialize)]
pub(super) struct PromptDetails {
    /// Tokens served from the cache.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) cached_tokens: Option<f64>,
    /// Tokens written to the cache.
    #[serde(default, deserialize_with = "lenient")]
    pub(super) cache_write_tokens: Option<f64>,
}
