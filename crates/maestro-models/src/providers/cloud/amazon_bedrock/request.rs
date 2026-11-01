//! The conversation request as the service input records spell it, and its one encoding.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::BedrockOptions;
use super::messages::{messages, system_blocks, tool_config};
use super::proxy::Environment;
use super::thinking::{additional_model_request_fields, cache_point, is_anthropic_claude_model};
use super::wire::{
    ContentBlock, PromptVariableValues, SystemContentBlock, ToolConfiguration, Union, list, map,
};
use crate::options::resolve_cache_retention;
use crate::providers::http::RequestFailure;
use crate::providers::json_text::compact_json;
use crate::{Context, Model};

/// One conversation turn.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Message {
    /// Who spoke.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) role: Option<String>,
    /// What was said.
    #[serde(deserialize_with = "list", skip_serializing_if = "Option::is_none")]
    pub(super) content: Option<Vec<Union<ContentBlock>>>,
}

/// Sampling and length settings.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Inference {
    /// Most tokens the answer may use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_tokens: Option<f64>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temperature: Option<f64>,
    /// Nucleus sampling mass.
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    /// Sequences that end the answer.
    #[serde(deserialize_with = "list", skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

/// Guardrail selection for a streamed answer.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Guardrail {
    /// Guardrail identifier.
    #[serde(
        rename = "guardrailIdentifier",
        skip_serializing_if = "Option::is_none"
    )]
    identifier: Option<String>,
    /// Guardrail version.
    #[serde(rename = "guardrailVersion", skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// Trace setting.
    #[serde(skip_serializing_if = "Option::is_none")]
    trace: Option<String>,
    /// Stream processing mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    stream_processing_mode: Option<String>,
}

/// Latency preference.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Performance {
    /// Latency mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    latency: Option<String>,
}

/// Service tier selection.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct ServiceTier {
    /// Tier name.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
}

/// A JSON schema the answer must follow.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct JsonSchema {
    /// Schema text, passed through as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    /// Schema name.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Schema description.
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// The structure an output format names.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct OutputFormatStructure {
    /// JSON schema structure.
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<JsonSchema>,
}

/// Output format of the answer text.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct TextFormat {
    /// Format kind.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    /// Format structure.
    #[serde(skip_serializing_if = "Option::is_none")]
    structure: Option<Union<OutputFormatStructure>>,
}

/// Output configuration.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct OutputConfig {
    /// Answer text format.
    #[serde(skip_serializing_if = "Option::is_none")]
    text_format: Option<TextFormat>,
}

/// The streamed conversation input without its model identifier. Members the service input does
/// not declare are dropped on decoding, and absent and `null` members are left out on encoding.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct Converse {
    /// Conversation turns.
    #[serde(deserialize_with = "list", skip_serializing_if = "Option::is_none")]
    pub(super) messages: Option<Vec<Message>>,
    /// System prompt blocks.
    #[serde(deserialize_with = "list", skip_serializing_if = "Option::is_none")]
    pub(super) system: Option<Vec<Union<SystemContentBlock>>>,
    /// Sampling and length settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) inference_config: Option<Inference>,
    /// Tool declarations and choice.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<ToolConfiguration>,
    /// Guardrail selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    guardrail_config: Option<Guardrail>,
    /// Model-specific request members, any JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    additional_model_request_fields: Option<Value>,
    /// Paths of model-specific response members to return.
    #[serde(deserialize_with = "list", skip_serializing_if = "Option::is_none")]
    additional_model_response_field_paths: Option<Vec<String>>,
    /// Prompt-variable values by name.
    #[serde(deserialize_with = "map", skip_serializing_if = "Option::is_none")]
    prompt_variables: Option<IndexMap<String, Union<PromptVariableValues>>>,
    /// Latency preference.
    #[serde(skip_serializing_if = "Option::is_none")]
    performance_config: Option<Performance>,
    /// Service tier.
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<ServiceTier>,
    /// Answer format.
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig>,
    /// Tags for cost allocation.
    #[serde(deserialize_with = "map", skip_serializing_if = "Option::is_none")]
    pub(super) request_metadata: Option<IndexMap<String, String>>,
}

impl Converse {
    /// Write the records as JSON with absent members left out; open documents are kept as they
    /// are, including any `null` inside them.
    fn encode(self) -> Result<Map<String, Value>, serde_json::Error> {
        Ok(match serde_json::to_value(self)? {
            Value::Object(members) => members,
            _ => Map::new(),
        })
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
/// Propagates [`messages`] conversion failures or fails when the conversation cannot be
/// written as JSON.
pub(super) fn payload<E: crate::ProviderExecution>(
    model: &Model,
    context: Context,
    options: &BedrockOptions<E>,
    env: &Environment,
) -> Result<Value, RequestFailure> {
    let cache = cache_point(
        model,
        resolve_cache_retention(options.common.cache_retention),
        env,
    );
    let Context {
        system_prompt,
        messages: history,
        tools,
    } = context;
    let converse = Converse {
        messages: Some(messages(model, &history, cache.as_ref())?),
        system: system_blocks(system_prompt, cache.as_ref()),
        inference_config: Some(Inference {
            max_tokens: options
                .common
                .max_tokens
                .or_else(|| is_anthropic_claude_model(model).then_some(model.max_tokens)),
            temperature: options.common.temperature,
            ..Inference::default()
        }),
        tool_config: tool_config(tools, options.tool_choice.as_ref()),
        additional_model_request_fields: additional_model_request_fields(model, options, env),
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
