//! Model descriptors with protocol compatibility and routing preferences.
use super::{Api, Provider, ThinkingLevelMap};
use serde::{Deserialize, Serialize};
/// Select the accepted completion token-limit field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    /// `MaxCompletionTokens`.
    MaxCompletionTokens,
    /// `MaxTokens`.
    MaxTokens,
}
/// Select the six accepted reasoning payload conventions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingFormat {
    /// Openai.
    Openai,
    /// Openrouter.
    Openrouter,
    /// Deepseek.
    Deepseek,
    /// Zai.
    Zai,
    /// Qwen.
    Qwen,
    /// `QwenChatTemplate`.
    #[serde(rename = "qwen-chat-template")]
    QwenChatTemplate,
}
/// Select the supported prompt-cache marker convention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheControlFormat {
    /// Anthropic.
    Anthropic,
}
/// Carry optional completion-protocol capability and routing overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenAICompletionsCompat {
    /// Supports store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_store: Option<bool>,
    /// Supports developer role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_developer_role: Option<bool>,
    /// Supports reasoning effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    /// Supports usage in streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_usage_in_streaming: Option<bool>,
    /// Max tokens field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<MaxTokensField>,
    /// Requires tool result name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_tool_result_name: Option<bool>,
    /// Requires assistant after tool result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_assistant_after_tool_result: Option<bool>,
    /// Requires thinking as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_thinking_as_text: Option<bool>,
    /// Requires reasoning content on assistant messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    /// Thinking format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<ThinkingFormat>,
    /// Open router routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_router_routing: Option<OpenRouterRouting>,
    /// Vercel gateway routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vercel_gateway_routing: Option<VercelGatewayRouting>,
    /// Zai tool stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zai_tool_stream: Option<bool>,
    /// Supports strict mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_strict_mode: Option<bool>,
    /// Cache control format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control_format: Option<CacheControlFormat>,
    /// Send session affinity headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_affinity_headers: Option<bool>,
    /// Supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// Carry optional response-protocol cache/header overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIResponsesCompat {
    /// Send session id header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_id_header: Option<bool>,
    /// Supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// Carry optional message-protocol tool-stream/cache overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessagesCompat {
    /// Supports eager tool input streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_eager_tool_input_streaming: Option<bool>,
    /// Supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// Carry the routing data-collection allow/deny choice.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataCollection {
    /// Deny.
    Deny,
    /// Allow.
    Allow,
}
/// Carry a routing sort string or by/partition record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoutingSort {
    /// Name.
    Name(String),
    /// Fields.
    Fields {
        /// By.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
        /// Partition.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "super::present"
        )]
        partition: Option<Option<String>>,
    },
}
/// Retain numeric or textual maximum-price values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoutingPrice {
    /// Number.
    Number(f64),
    /// Text.
    Text(String),
}
/// Carry independently optional per-unit maximum prices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct MaxPrice {
    /// Prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<RoutingPrice>,
    /// Completion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion: Option<RoutingPrice>,
    /// Image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<RoutingPrice>,
    /// Audio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<RoutingPrice>,
    /// Request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RoutingPrice>,
}
/// Carry a scalar or percentile-specific routing threshold.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RoutingThreshold {
    /// Number.
    Number(f64),
    /// Percentiles.
    Percentiles {
        /// P50.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p50: Option<f64>,
        /// P75.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p75: Option<f64>,
        /// P90.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p90: Option<f64>,
        /// P99.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        p99: Option<f64>,
    },
}
/// Carry upstream selection, privacy, order, price and performance preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct OpenRouterRouting {
    /// Allow fallbacks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    /// Require parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    /// Data collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<DataCollection>,
    /// Zdr.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    /// Enforce distillable text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    /// Order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// Ignore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    /// Quantizations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<String>>,
    /// Sort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<RoutingSort>,
    /// Max price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
    /// Preferred min throughput.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_min_throughput: Option<RoutingThreshold>,
    /// Preferred max latency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preferred_max_latency: Option<RoutingThreshold>,
}
/// Carry gateway provider allow/order lists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct VercelGatewayRouting {
    /// Only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// Order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
}
/// Represent supported text and image inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelInput {
    /// Text.
    Text,
    /// Image.
    Image,
}
/// Carry the four supplied per-million token rates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCost {
    /// Input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache read.
    pub cache_read: f64,
    /// Cache write.
    pub cache_write: f64,
}
/// Select typed compatibility fields using the model protocol identifier.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ModelCompat {
    /// `OpenAICompletions`.
    OpenAICompletions(Box<OpenAICompletionsCompat>),
    /// `OpenAIResponses`.
    OpenAIResponses(OpenAIResponsesCompat),
    /// `AnthropicMessages`.
    AnthropicMessages(AnthropicMessagesCompat),
}
/// Supply an invocable model descriptor independently of catalog membership.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", remote = "Self")]
pub struct Model {
    /// Id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Api.
    pub api: Api,
    /// Provider.
    pub provider: Provider,
    /// Base url.
    pub base_url: String,
    /// Reasoning.
    pub reasoning: bool,
    /// Thinking level map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<ThinkingLevelMap>,
    /// Input.
    pub input: Vec<ModelInput>,
    /// Cost.
    pub cost: ModelCost,
    /// Context window.
    pub context_window: f64,
    /// Max tokens.
    pub max_tokens: f64,
    /// Headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    /// Compat.
    #[serde(default, skip_serializing_if = "Option::is_none", skip_deserializing)]
    pub compat: Option<ModelCompat>,
}

impl Serialize for Model {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for Model {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let mut wire = serde_json::Value::deserialize(deserializer)?;
        let compat = wire
            .as_object_mut()
            .and_then(|object| object.remove("compat"));
        let mut model = Self::deserialize(wire).map_err(serde::de::Error::custom)?;
        model.compat = compat
            .map(|wire| decode_compat(&model.api, wire))
            .transpose()
            .map_err(serde::de::Error::custom)?;
        Ok(model)
    }
}
fn decode_compat(api: &str, wire: serde_json::Value) -> Result<ModelCompat, serde_json::Error> {
    match api {
        "openai-completions" => serde_json::from_value(wire).map(ModelCompat::OpenAICompletions),
        "openai-responses" => serde_json::from_value(wire).map(ModelCompat::OpenAIResponses),
        "anthropic-messages" => serde_json::from_value(wire).map(ModelCompat::AnthropicMessages),
        _ => Err(serde::de::Error::custom(
            "compatibility fields require a supported protocol",
        )),
    }
}
