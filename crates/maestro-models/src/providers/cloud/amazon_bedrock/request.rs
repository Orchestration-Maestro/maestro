//! The conversation request as the service input records spell it, and its one encoding.

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::BedrockOptions;
use super::messages::{system_blocks, user_messages};
use crate::providers::http::RequestFailure;
use crate::providers::json_text::compact_json;
use crate::{Context, Model};

/// A list that keeps its present entries, whatever a `null` entry stood for.
fn list<'de, D, T>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Option::<Vec<Option<T>>>::deserialize(deserializer)?;
    Ok(items.map(|items| items.into_iter().flatten().collect()))
}

/// A map that keeps its present entries, whatever a `null` value stood for.
fn map<'de, D, T>(deserializer: D) -> Result<Option<IndexMap<String, T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let items = Option::<IndexMap<String, Option<T>>>::deserialize(deserializer)?;
    Ok(items.map(|items| {
        items
            .into_iter()
            .filter_map(|(name, value)| Some((name, value?)))
            .collect()
    }))
}

/// One text member of a content, system or prompt-variable union.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct TextBlock {
    /// The text.
    pub(super) text: Option<String>,
}

/// One conversation turn.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Message {
    /// Who spoke.
    pub(super) role: Option<String>,
    /// What was said.
    #[serde(deserialize_with = "list")]
    pub(super) content: Option<Vec<TextBlock>>,
}

/// Sampling and length settings.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Inference {
    /// Most tokens the answer may use.
    pub(super) max_tokens: Option<f64>,
    /// Sampling temperature.
    pub(super) temperature: Option<f64>,
    /// Nucleus sampling mass.
    top_p: Option<f64>,
    /// Sequences that end the answer.
    #[serde(deserialize_with = "list")]
    stop_sequences: Option<Vec<String>>,
}

/// Guardrail selection for a streamed answer.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Guardrail {
    /// Guardrail identifier.
    #[serde(rename = "guardrailIdentifier")]
    identifier: Option<String>,
    /// Guardrail version.
    #[serde(rename = "guardrailVersion")]
    version: Option<String>,
    /// Trace setting.
    trace: Option<String>,
    /// Stream processing mode.
    stream_processing_mode: Option<String>,
}

/// Latency preference.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Performance {
    /// Latency mode.
    latency: Option<String>,
}

/// Service tier selection.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct ServiceTier {
    /// Tier name.
    #[serde(rename = "type")]
    kind: Option<String>,
}

/// A JSON schema the answer must follow.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct JsonSchema {
    /// Schema text, passed through as written.
    schema: Option<String>,
    /// Schema name.
    name: Option<String>,
    /// Schema description.
    description: Option<String>,
}

/// The structure an output format names.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Structure {
    /// JSON schema structure.
    json_schema: Option<JsonSchema>,
}

/// Output format of the answer text.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct TextFormat {
    /// Format kind.
    #[serde(rename = "type")]
    kind: Option<String>,
    /// Format structure.
    structure: Option<Structure>,
}

/// Output configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct OutputConfig {
    /// Answer text format.
    text_format: Option<TextFormat>,
}

/// The streamed conversation input without its model identifier. Members the service input does
/// not declare are dropped on decoding, and absent and `null` members are left out on encoding.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Converse {
    /// Conversation turns.
    #[serde(deserialize_with = "list")]
    pub(super) messages: Option<Vec<Message>>,
    /// System prompt blocks.
    #[serde(deserialize_with = "list")]
    pub(super) system: Option<Vec<TextBlock>>,
    /// Sampling and length settings.
    pub(super) inference_config: Option<Inference>,
    /// Guardrail selection.
    guardrail_config: Option<Guardrail>,
    /// Model-specific request members, any JSON.
    additional_model_request_fields: Option<Value>,
    /// Paths of model-specific response members to return.
    #[serde(deserialize_with = "list")]
    additional_model_response_field_paths: Option<Vec<String>>,
    /// Prompt-variable values by name.
    #[serde(deserialize_with = "map")]
    prompt_variables: Option<IndexMap<String, TextBlock>>,
    /// Latency preference.
    performance_config: Option<Performance>,
    /// Service tier.
    service_tier: Option<ServiceTier>,
    /// Answer format.
    output_config: Option<OutputConfig>,
    /// Tags for cost allocation.
    #[serde(deserialize_with = "map")]
    pub(super) request_metadata: Option<IndexMap<String, String>>,
}

/// Remove the members that hold `null`, which a record writes for each absent member.
fn strip_absent(value: &mut Value) {
    match value {
        Value::Object(members) => {
            members.retain(|_, member| !member.is_null());
            members.values_mut().for_each(strip_absent);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_absent),
        _ => {}
    }
}

impl Converse {
    /// Write the records as JSON with absent members left out; the open document is kept as is,
    /// including any `null` inside it.
    fn encode(mut self) -> Result<Map<String, Value>, serde_json::Error> {
        let document = self.additional_model_request_fields.take();
        let mut value = serde_json::to_value(self)?;
        strip_absent(&mut value);
        let mut members = match value {
            Value::Object(members) => members,
            _ => Map::new(),
        };
        if let Some(document) = document {
            members.insert("additionalModelRequestFields".to_owned(), document);
        }
        Ok(members)
    }
}

/// What the service route and body take from the request after the hook ran.
pub(super) struct Prepared {
    /// Model identifier for the route; absent when the request names none.
    pub(super) model_id: Option<String>,
    /// Encoded JSON body; empty when the route cannot be bound.
    pub(super) body: Vec<u8>,
}

/// The input the payload hook sees: the model identifier first, then the conversation.
///
/// # Errors
/// Fails when the conversation cannot be written as JSON.
pub(super) fn payload(
    model: &Model,
    context: &Context,
    options: &BedrockOptions,
) -> Result<Value, RequestFailure> {
    let converse = Converse {
        messages: Some(user_messages(context)),
        system: system_blocks(context),
        inference_config: Some(Inference {
            max_tokens: options.common.max_tokens,
            temperature: options.common.temperature,
            ..Inference::default()
        }),
        request_metadata: options.request_metadata.clone(),
        ..Converse::default()
    };
    let mut input = Map::new();
    input.insert("modelId".to_owned(), Value::String(model.id.clone()));
    input.extend(converse.encode().map_err(|error| failure(&error))?);
    Ok(Value::Object(input))
}

/// Select the route and encode the body from the payload the hook returned. The model
/// identifier is read before the rest is decoded, so a request that names none fails at the
/// route whatever else is malformed.
///
/// # Errors
/// Fails when the rest of the payload is not a conversation input or cannot be written as JSON.
pub(super) fn prepare(mut input: Value) -> Result<Prepared, RequestFailure> {
    let model_id = input
        .get("modelId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    if model_id.is_none() {
        return Ok(Prepared {
            model_id,
            body: Vec::new(),
        });
    }
    if let Value::Object(members) = &mut input {
        members.shift_remove("modelId");
    }
    let converse: Converse = serde_json::from_value(input).map_err(|error| failure(&error))?;
    let body = converse.encode().map_err(|error| failure(&error))?;
    let body = compact_json(&Value::Object(body)).map_err(|error| failure(&error))?;
    Ok(Prepared {
        model_id,
        body: body.into_bytes(),
    })
}

/// Report a conversion failure with its native text.
fn failure(error: &serde_json::Error) -> RequestFailure {
    RequestFailure::new(error.to_string())
}
