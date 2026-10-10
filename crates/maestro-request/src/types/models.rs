//! Model descriptors with protocol compatibility and routing preferences.
use super::{Api, JsonObject, Provider, ThinkingLevelMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Select the accepted completion token-limit field.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    /// `MaxCompletionTokens`.
    MaxCompletionTokens,
    /// `MaxTokens`.
    MaxTokens,
}
/// Select the six accepted reasoning payload conventions.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
/// Retain the complete ordered compatibility-option object independently of protocol.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelCompat(pub JsonObject);
/// Supply an invocable model descriptor independently of catalog membership.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    pub headers: Option<indexmap::IndexMap<String, String>>,
    /// Compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compat: Option<ModelCompat>,
}

impl From<OpenAICompletionsCompat> for ModelCompat {
    fn from(value: OpenAICompletionsCompat) -> Self {
        let mut object = completion_prefix(&value);
        insert(
            &mut object,
            "openRouterRouting",
            value.open_router_routing.map(router),
        );
        insert(
            &mut object,
            "vercelGatewayRouting",
            value.vercel_gateway_routing.map(gateway),
        );
        insert(&mut object, "zaiToolStream", value.zai_tool_stream);
        insert(
            &mut object,
            "supportsStrictMode",
            value.supports_strict_mode,
        );
        insert(
            &mut object,
            "cacheControlFormat",
            value.cache_control_format.map(cache_control_format),
        );
        insert(
            &mut object,
            "sendSessionAffinityHeaders",
            value.send_session_affinity_headers,
        );
        insert(
            &mut object,
            "supportsLongCacheRetention",
            value.supports_long_cache_retention,
        );
        Self(object)
    }
}

impl From<OpenAIResponsesCompat> for ModelCompat {
    fn from(value: OpenAIResponsesCompat) -> Self {
        let mut object = JsonObject::new();
        insert(
            &mut object,
            "sendSessionIdHeader",
            value.send_session_id_header,
        );
        insert(
            &mut object,
            "supportsLongCacheRetention",
            value.supports_long_cache_retention,
        );
        Self(object)
    }
}

impl From<AnthropicMessagesCompat> for ModelCompat {
    fn from(value: AnthropicMessagesCompat) -> Self {
        let mut object = JsonObject::new();
        insert(
            &mut object,
            "supportsEagerToolInputStreaming",
            value.supports_eager_tool_input_streaming,
        );
        insert(
            &mut object,
            "supportsLongCacheRetention",
            value.supports_long_cache_retention,
        );
        Self(object)
    }
}

/// Construct the supplied routing members in declaration order.
fn router(value: OpenRouterRouting) -> Value {
    let mut object = JsonObject::new();
    insert(&mut object, "allow_fallbacks", value.allow_fallbacks);
    insert(&mut object, "require_parameters", value.require_parameters);
    insert(
        &mut object,
        "data_collection",
        value.data_collection.as_ref().map(data_collection),
    );
    insert(&mut object, "zdr", value.zdr);
    insert(
        &mut object,
        "enforce_distillable_text",
        value.enforce_distillable_text,
    );
    insert(&mut object, "order", value.order);
    insert(&mut object, "only", value.only);
    insert(&mut object, "ignore", value.ignore);
    insert(&mut object, "quantizations", value.quantizations);
    insert(&mut object, "sort", value.sort.map(sort));
    insert(&mut object, "max_price", value.max_price.map(prices));
    insert(
        &mut object,
        "preferred_min_throughput",
        value.preferred_min_throughput.as_ref().map(threshold),
    );
    insert(
        &mut object,
        "preferred_max_latency",
        value.preferred_max_latency.as_ref().map(threshold),
    );
    Value::Object(object)
}

/// Construct the supplied routing members in declaration order.
fn gateway(value: VercelGatewayRouting) -> Value {
    let mut object = JsonObject::new();
    insert(&mut object, "only", value.only);
    insert(&mut object, "order", value.order);
    Value::Object(object)
}

/// Construct the supplied routing members in declaration order.
fn prices(value: MaxPrice) -> Value {
    let mut object = JsonObject::new();
    insert(&mut object, "prompt", value.prompt.map(price));
    insert(&mut object, "completion", value.completion.map(price));
    insert(&mut object, "image", value.image.map(price));
    insert(&mut object, "audio", value.audio.map(price));
    insert(&mut object, "request", value.request.map(price));
    Value::Object(object)
}

/// Insert only supplied optional fields.
fn insert<T: Into<Value>>(object: &mut JsonObject, key: &str, value: Option<T>) {
    if let Some(value) = value {
        object.insert(key.into(), value.into());
    }
}
/// Encode the exact token-field literal.
fn max_tokens_field(value: MaxTokensField) -> Value {
    Value::from(match value {
        MaxTokensField::MaxCompletionTokens => "max_completion_tokens",
        MaxTokensField::MaxTokens => "max_tokens",
    })
}
/// Encode the exact reasoning convention literal.
fn thinking_format(value: ThinkingFormat) -> Value {
    Value::from(match value {
        ThinkingFormat::Openai => "openai",
        ThinkingFormat::Openrouter => "openrouter",
        ThinkingFormat::Deepseek => "deepseek",
        ThinkingFormat::Zai => "zai",
        ThinkingFormat::Qwen => "qwen",
        ThinkingFormat::QwenChatTemplate => "qwen-chat-template",
    })
}
/// Encode the cache marker literal.
fn cache_control_format(value: CacheControlFormat) -> Value {
    match value {
        CacheControlFormat::Anthropic => Value::from("anthropic"),
    }
}
/// Encode the routing privacy literal.
fn data_collection(value: &DataCollection) -> Value {
    Value::from(match value {
        DataCollection::Allow => "allow",
        DataCollection::Deny => "deny",
    })
}
/// Encode a routing sort, retaining explicit null partitions.
fn sort(value: RoutingSort) -> Value {
    match value {
        RoutingSort::Name(name) => name.into(),
        RoutingSort::Fields { by, partition } => {
            let mut object = JsonObject::new();
            insert(&mut object, "by", by);
            insert(
                &mut object,
                "partition",
                partition.map(|value| value.map_or(Value::Null, Value::from)),
            );
            Value::Object(object)
        }
    }
}
/// Encode numeric prices with native nonfinite handling.
fn price(value: RoutingPrice) -> Value {
    match value {
        RoutingPrice::Number(number) => number.into(),
        RoutingPrice::Text(text) => text.into(),
    }
}
/// Encode scalar or percentile thresholds.
fn threshold(value: &RoutingThreshold) -> Value {
    match value {
        RoutingThreshold::Number(number) => (*number).into(),
        RoutingThreshold::Percentiles { p50, p75, p90, p99 } => {
            let mut object = JsonObject::new();
            insert(&mut object, "p50", *p50);
            insert(&mut object, "p75", *p75);
            insert(&mut object, "p90", *p90);
            insert(&mut object, "p99", *p99);
            Value::Object(object)
        }
    }
}

/// Construct completion capability members preceding routing preferences.
fn completion_prefix(value: &OpenAICompletionsCompat) -> JsonObject {
    let mut object = JsonObject::new();
    insert(&mut object, "supportsStore", value.supports_store);
    insert(
        &mut object,
        "supportsDeveloperRole",
        value.supports_developer_role,
    );
    insert(
        &mut object,
        "supportsReasoningEffort",
        value.supports_reasoning_effort,
    );
    insert(
        &mut object,
        "supportsUsageInStreaming",
        value.supports_usage_in_streaming,
    );
    insert(
        &mut object,
        "maxTokensField",
        value.max_tokens_field.map(max_tokens_field),
    );
    insert(
        &mut object,
        "requiresToolResultName",
        value.requires_tool_result_name,
    );
    insert(
        &mut object,
        "requiresAssistantAfterToolResult",
        value.requires_assistant_after_tool_result,
    );
    insert(
        &mut object,
        "requiresThinkingAsText",
        value.requires_thinking_as_text,
    );
    insert(
        &mut object,
        "requiresReasoningContentOnAssistantMessages",
        value.requires_reasoning_content_on_assistant_messages,
    );
    insert(
        &mut object,
        "thinkingFormat",
        value.thinking_format.map(thinking_format),
    );
    object
}
