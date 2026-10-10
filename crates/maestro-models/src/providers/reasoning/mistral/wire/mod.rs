//! Typed mod transport records.

use guardrails::GuardrailConfig;
use messages::Message;
use serde::{Deserialize, Serialize};
use tools::{RequestTool, ToolSelection};

/// Transport fields for `ChatCompletionStreamRequest`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
struct Request {
    /// The `model` field.
    model: String,
    /// The `temperature` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    temperature: Option<Nullable<serde_json::Number>>,
    /// The `topP` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    top_p: Option<Nullable<serde_json::Number>>,
    /// The `maxTokens` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    max_tokens: Option<Nullable<SafeInteger>>,
    /// The `stream` field.
    #[serde(default = "request_stream_default")]
    stream: bool,
    /// The `stop` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    stop: Option<Nullable<StringOrList>>,
    /// The `randomSeed` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    random_seed: Option<Nullable<SafeInteger>>,
    /// The `metadata` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    metadata: Option<Nullable<serde_json::Map<String, serde_json::Value>>>,
    /// The `messages` field.
    messages: Vec<Message>,
    /// The `responseFormat` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    response_format: Option<Object<ResponseFormat>>,
    /// The `tools` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    tools: Option<Nullable<Vec<RequestTool>>>,
    /// The `toolChoice` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    tool_choice: Option<ToolSelection>,
    /// The `presencePenalty` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    presence_penalty: Option<Nullable<serde_json::Number>>,
    /// The `frequencyPenalty` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    frequency_penalty: Option<Nullable<serde_json::Number>>,
    /// The `n` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    n: Option<Nullable<SafeInteger>>,
    /// The `prediction` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    prediction: Option<Object<Prediction>>,
    /// The `parallelToolCalls` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    parallel_tool_calls: Option<bool>,
    /// The `reasoningEffort` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    reasoning_effort: Option<Nullable<String>>,
    /// The `promptMode` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    prompt_mode: Option<Nullable<String>>,
    /// The `guardrails` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    guardrails: Option<Nullable<Vec<Object<GuardrailConfig>>>>,
    /// The `safePrompt` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    safe_prompt: Option<bool>,
}

/// Transport fields for `ResponseFormat`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
struct ResponseFormat {
    /// The `type` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none",
        rename = "type"
    )]
    kind: Option<String>,
    /// The `jsonSchema` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    json_schema: Option<Nullable<Object<JsonSchema>>>,
}

/// Transport fields for `JsonSchema`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
struct JsonSchema {
    /// The `name` field.
    name: String,
    /// The `description` field.
    #[serde(
        default,
        deserialize_with = "nullable",
        skip_serializing_if = "Option::is_none"
    )]
    description: Option<Nullable<String>>,
    /// The `schemaDefinition` field.
    #[serde(rename(serialize = "schema"))]
    schema_definition: serde_json::Map<String, serde_json::Value>,
    /// The `strict` field.
    #[serde(
        default,
        deserialize_with = "optional",
        skip_serializing_if = "Option::is_none"
    )]
    strict: Option<bool>,
}

/// Transport fields for `Prediction`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all(deserialize = "camelCase", serialize = "snake_case"))]
struct Prediction {
    /// The `type` field.
    #[serde(default, rename = "type")]
    #[serde(deserialize_with = "literal")]
    kind: PredictionTag,
    /// The `content` field.
    #[serde(default)]
    content: String,
}

/// The declared field default.
fn request_stream_default() -> bool {
    true
}

/// Literal `content` tag.
#[derive(Default, Deserialize, Serialize)]
pub(super) enum PredictionTag {
    /// Transport discriminator.
    #[default]
    #[serde(rename = "content")]
    Value,
}

mod content;
mod guardrails;
mod messages;
mod tools;

use crate::providers::http::RequestFailure;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserializer, de::value::MapAccessDeserializer};
use serde_json::{Number, Value};
use std::fmt;

/// Encode the selected typed transport fields.
pub(super) fn encode_payload(payload: &Value) -> Result<Value, RequestFailure> {
    let result = Object::<Request>::deserialize(payload).and_then(serde_json::to_value);
    result.map_err(|error| RequestFailure::new(format!("Input validation failed: {error}")))
}

/// A record that accepts only object input.
#[derive(Serialize)]
#[serde(transparent)]
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Decode a map directly into its typed record.
        struct ObjectVisitor<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
            type Value = Object<T>;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object")
            }
            fn visit_map<M: MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(Object)
            }
        }
        deserializer.deserialize_map(ObjectVisitor(std::marker::PhantomData))
    }
}

/// Preserve absence separately from admitted null.
pub(super) fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Nullable<T>>, D::Error> {
    Nullable::<T>::deserialize(deserializer).map(Some)
}

/// Decode an optional field that does not admit null.
pub(super) fn optional<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Require a nullable field to be present.
pub(super) fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}

/// An integral number in the safe integer interval.
#[derive(Serialize)]
#[serde(transparent)]
pub(super) struct SafeInteger(pub(super) Number);

impl Default for SafeInteger {
    fn default() -> Self {
        Self(Number::from(0))
    }
}

impl<'de> Deserialize<'de> for SafeInteger {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = Number::deserialize(deserializer)?;
        let value = number
            .as_f64()
            .ok_or_else(|| D::Error::custom("expected a finite safe integer"))?;
        if value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
            return Err(D::Error::custom("expected a safe integer"));
        }
        Ok(Self(number))
    }
}

/// A string or a list of strings.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum StringOrList {
    /// One string.
    String(String),
    /// Multiple strings.
    List(Vec<String>),
}

/// Decode literal tags only from strings, not numeric variant indices.
fn literal<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
    deserializer: D,
) -> Result<T, D::Error> {
    use serde::de::IntoDeserializer;
    let text = String::deserialize(deserializer)?;
    T::deserialize(text.into_deserializer())
}

/// A present literal tag must be a string; absence remains distinct.
fn optional_literal<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    literal(deserializer).map(Some)
}

/// The value domain of a nullable field, separate from its presence.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum Nullable<T> {
    /// Explicit JSON null.
    Null,
    /// Typed field value.
    Value(T),
}
