//! Supplied wire records and callable options.
use super::diagnostics::AssistantMessageDiagnostic;
use std::sync::{Arc, RwLock};
/// KnownApi supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownApi {
    /// OpenaiCompletions value.
    OpenaiCompletions,
    /// MistralConversations value.
    MistralConversations,
    /// OpenaiResponses value.
    OpenaiResponses,
    /// AzureOpenaiResponses value.
    AzureOpenaiResponses,
    /// OpenaiCodexResponses value.
    OpenaiCodexResponses,
    /// AnthropicMessages value.
    AnthropicMessages,
    /// BedrockConverseStream value.
    BedrockConverseStream,
    /// GoogleGenerativeAi value.
    GoogleGenerativeAi,
    /// GoogleVertex value.
    GoogleVertex,
}
/// Api supplied record.
pub type Api = String;
/// KnownProvider supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownProvider {
    /// AmazonBedrock value.
    AmazonBedrock,
    /// Anthropic value.
    Anthropic,
    /// Google value.
    Google,
    /// GoogleVertex value.
    GoogleVertex,
    /// Openai value.
    Openai,
    /// AzureOpenaiResponses value.
    AzureOpenaiResponses,
    /// OpenaiCodex value.
    OpenaiCodex,
    /// Deepseek value.
    Deepseek,
    /// GithubCopilot value.
    GithubCopilot,
    /// Xai value.
    Xai,
    /// Groq value.
    Groq,
    /// Cerebras value.
    Cerebras,
    /// Openrouter value.
    Openrouter,
    /// VercelAiGateway value.
    VercelAiGateway,
    /// Zai value.
    Zai,
    /// Mistral value.
    Mistral,
    /// Minimax value.
    Minimax,
    /// MinimaxCn value.
    MinimaxCn,
    /// Moonshotai value.
    Moonshotai,
    /// MoonshotaiCn value.
    MoonshotaiCn,
    /// Huggingface value.
    Huggingface,
    /// Fireworks value.
    Fireworks,
    /// Opencode value.
    Opencode,
    /// OpencodeGo value.
    OpencodeGo,
    /// KimiCoding value.
    KimiCoding,
    /// CloudflareWorkersAi value.
    CloudflareWorkersAi,
    /// CloudflareAiGateway value.
    CloudflareAiGateway,
    /// Xiaomi value.
    Xiaomi,
    /// XiaomiTokenPlanCn value.
    XiaomiTokenPlanCn,
    /// XiaomiTokenPlanAms value.
    XiaomiTokenPlanAms,
    /// XiaomiTokenPlanSgp value.
    XiaomiTokenPlanSgp,
}
/// Provider supplied record.
pub type Provider = String;
/// ThinkingLevel supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThinkingLevel {
    /// Minimal value.
    Minimal,
    /// Low value.
    Low,
    /// Medium value.
    Medium,
    /// High value.
    High,
    /// Xhigh value.
    Xhigh,
}
/// ModelThinkingLevel supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelThinkingLevel {
    /// Off value.
    Off,
    /// Minimal value.
    Minimal,
    /// Low value.
    Low,
    /// Medium value.
    Medium,
    /// High value.
    High,
    /// Xhigh value.
    Xhigh,
}
/// ThinkingLevelMap supplied record.
pub type ThinkingLevelMap = serde_json::Map<String, serde_json::Value>;
/// ThinkingBudgets supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingBudgets {
    /// Supplied minimal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimal: Option<f64>,
    /// Supplied low.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    /// Supplied medium.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<f64>,
    /// Supplied high.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
}
/// CacheRetention supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheRetention {
    /// None value.
    None,
    /// Short value.
    Short,
    /// Long value.
    Long,
}
/// Transport supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    /// Sse value.
    Sse,
    /// Websocket value.
    Websocket,
    /// WebsocketCached value.
    #[serde(rename = "websocket-cached")]
    WebsocketCached,
    /// Auto value.
    Auto,
}
/// ProviderResponse supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    /// Supplied status.
    pub status: f64,
    /// Supplied headers.
    #[serde(serialize_with = "serialize_json")]
    pub headers: serde_json::Map<String, serde_json::Value>,
}
/// TextSignatureV1 supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSignatureV1 {
    /// Supplied v.
    pub v: u8,
    /// Supplied id.
    pub id: String,
    /// Supplied phase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}
/// TextContent supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename = "text", rename_all = "camelCase")]
pub struct TextContent {
    /// Supplied text.
    pub text: String,
    /// Supplied text signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_signature: Option<String>,
}
/// ThinkingContent supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename = "thinking", rename_all = "camelCase")]
pub struct ThinkingContent {
    /// Supplied thinking.
    pub thinking: String,
    /// Supplied thinking signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_signature: Option<String>,
    /// Supplied redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted: Option<bool>,
}
/// ImageContent supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename = "image", rename_all = "camelCase")]
pub struct ImageContent {
    /// Supplied data.
    pub data: String,
    /// Supplied mime type.
    pub mime_type: String,
}
/// ToolCall supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename = "toolCall", rename_all = "camelCase")]
pub struct ToolCall {
    /// Supplied id.
    pub id: String,
    /// Supplied name.
    pub name: String,
    /// Supplied arguments.
    #[serde(serialize_with = "serialize_map")]
    pub arguments: serde_json::Map<String, serde_json::Value>,
    /// Supplied thought signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}
/// InputContent supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum InputContent {
    /// Text value.
    Text(TextContent),
    /// Image value.
    Image(ImageContent),
}
/// AssistantContent supplied record.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum AssistantContent {
    /// Text value.
    Text(TextContent),
    /// Thinking value.
    Thinking(ThinkingContent),
    /// ToolCall value.
    ToolCall(#[serde(with = "locked")] Arc<RwLock<ToolCall>>),
}
/// UserContent supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    /// Text value.
    Text(String),
    /// Blocks value.
    Blocks(Vec<InputContent>),
}
/// TokenRates supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenRates {
    /// Supplied input.
    pub input: f64,
    /// Supplied output.
    pub output: f64,
    /// Supplied cache read.
    pub cache_read: f64,
    /// Supplied cache write.
    pub cache_write: f64,
}
/// UsageCost supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageCost {
    /// Supplied input.
    pub input: f64,
    /// Supplied output.
    pub output: f64,
    /// Supplied cache read.
    pub cache_read: f64,
    /// Supplied cache write.
    pub cache_write: f64,
    /// Supplied total.
    pub total: f64,
}
/// Usage supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Supplied input.
    pub input: f64,
    /// Supplied output.
    pub output: f64,
    /// Supplied cache read.
    pub cache_read: f64,
    /// Supplied cache write.
    pub cache_write: f64,
    /// Supplied total tokens.
    pub total_tokens: f64,
    /// Supplied cost.
    pub cost: UsageCost,
}
/// StopReason supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Stop value.
    Stop,
    /// Length value.
    Length,
    /// ToolUse value.
    ToolUse,
    /// Error value.
    Error,
    /// Aborted value.
    Aborted,
}
/// UserMessage supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role", rename = "user", rename_all = "camelCase")]
pub struct UserMessage {
    /// Supplied content.
    pub content: UserContent,
    /// Supplied timestamp.
    pub timestamp: f64,
}
/// AssistantMessage supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role", rename = "assistant", rename_all = "camelCase")]
pub struct AssistantMessage {
    /// Supplied content.
    pub content: Vec<AssistantContent>,
    /// Supplied api.
    pub api: Api,
    /// Supplied provider.
    pub provider: Provider,
    /// Supplied model.
    pub model: String,
    /// Supplied response model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_model: Option<String>,
    /// Supplied response id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    /// Supplied diagnostics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<Vec<AssistantMessageDiagnostic>>,
    /// Supplied usage.
    pub usage: Usage,
    /// Supplied stop reason.
    pub stop_reason: StopReason,
    /// Supplied error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Supplied timestamp.
    pub timestamp: f64,
}
/// ToolResultMessage supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "role", rename = "toolResult", rename_all = "camelCase")]
pub struct ToolResultMessage<TDetails = serde_json::Value> {
    /// Supplied tool call id.
    pub tool_call_id: String,
    /// Supplied tool name.
    pub tool_name: String,
    /// Supplied content.
    pub content: Vec<InputContent>,
    /// Supplied details.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(
        deserialize_with = "present_value",
        bound(deserialize = "TDetails: serde::Deserialize<'de>")
    )]
    #[serde(
        serialize_with = "serialize_json",
        bound(serialize = "TDetails: serde::Serialize")
    )]
    pub details: Option<TDetails>,
    /// Supplied is error.
    pub is_error: bool,
    /// Supplied timestamp.
    pub timestamp: f64,
}
/// Message supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(untagged)]
pub enum Message {
    /// User value.
    User(UserMessage),
    /// Assistant value.
    Assistant(AssistantMessage),
    /// ToolResult value.
    ToolResult(ToolResultMessage),
}
impl<'de> serde::Deserialize<'de> for Message {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(tag = "role", rename_all = "camelCase")]
        enum ByRole {
            User(UserMessage),
            Assistant(AssistantMessage),
            ToolResult(ToolResultMessage),
        }
        Ok(match ByRole::deserialize(deserializer)? {
            ByRole::User(value) => Self::User(value),
            ByRole::Assistant(value) => Self::Assistant(value),
            ByRole::ToolResult(value) => Self::ToolResult(value),
        })
    }
}
/// Tool supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool<TParameters = serde_json::Value> {
    /// Supplied name.
    pub name: String,
    /// Supplied description.
    pub description: String,
    /// Supplied parameters.
    #[serde(
        serialize_with = "serialize_json",
        bound(serialize = "TParameters: serde::Serialize")
    )]
    pub parameters: TParameters,
}
/// Context supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    /// Supplied system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Supplied messages.
    pub messages: Vec<Message>,
    /// Supplied tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}
/// AssistantMessageEvent supplied record.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AssistantMessageEvent {
    /// Start value.
    Start {
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// TextStart value.
    TextStart {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// TextDelta value.
    TextDelta {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied delta.
        delta: String,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// TextEnd value.
    TextEnd {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied content.
        content: String,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ThinkingStart value.
    ThinkingStart {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ThinkingDelta value.
    ThinkingDelta {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied delta.
        delta: String,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ThinkingEnd value.
    ThinkingEnd {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied content.
        content: String,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ToolcallStart value.
    ToolcallStart {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ToolcallDelta value.
    ToolcallDelta {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied delta.
        delta: String,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// ToolcallEnd value.
    ToolcallEnd {
        /// Supplied content_index.
        content_index: f64,
        /// Supplied tool_call.
        #[serde(with = "locked")]
        tool_call: Arc<RwLock<ToolCall>>,
        /// Supplied partial.
        #[serde(with = "locked")]
        partial: Arc<RwLock<AssistantMessage>>,
    },
    /// Done value.
    Done {
        /// Supplied reason.
        reason: StopReason,
        /// Supplied message.
        #[serde(with = "locked")]
        message: Arc<RwLock<AssistantMessage>>,
    },
    /// Error value.
    Error {
        /// Supplied reason.
        reason: StopReason,
        /// Supplied error.
        #[serde(with = "locked")]
        error: Arc<RwLock<AssistantMessage>>,
    },
}
/// OpenAICompletionsCompat supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAICompletionsCompat {
    /// Supplied supports store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_store: Option<bool>,
    /// Supplied supports developer role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_developer_role: Option<bool>,
    /// Supplied supports reasoning effort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_effort: Option<bool>,
    /// Supplied supports usage in streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_usage_in_streaming: Option<bool>,
    /// Supplied max tokens field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens_field: Option<String>,
    /// Supplied requires tool result name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_tool_result_name: Option<bool>,
    /// Supplied requires assistant after tool result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_assistant_after_tool_result: Option<bool>,
    /// Supplied requires thinking as text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_thinking_as_text: Option<bool>,
    /// Supplied requires reasoning content on assistant messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    /// Supplied thinking format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_format: Option<String>,
    /// Supplied open router routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open_router_routing: Option<OpenRouterRouting>,
    /// Supplied vercel gateway routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vercel_gateway_routing: Option<VercelGatewayRouting>,
    /// Supplied zai tool stream.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zai_tool_stream: Option<bool>,
    /// Supplied supports strict mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_strict_mode: Option<bool>,
    /// Supplied cache control format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_control_format: Option<String>,
    /// Supplied send session affinity headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_affinity_headers: Option<bool>,
    /// Supplied supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// OpenAIResponsesCompat supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIResponsesCompat {
    /// Supplied send session id header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send_session_id_header: Option<bool>,
    /// Supplied supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// AnthropicMessagesCompat supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnthropicMessagesCompat {
    /// Supplied supports eager tool input streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_eager_tool_input_streaming: Option<bool>,
    /// Supplied supports long cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_long_cache_retention: Option<bool>,
}
/// OpenRouterRouting supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OpenRouterRouting {
    /// Supplied allow fallbacks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    /// Supplied require parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    /// Supplied data collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<String>,
    /// Supplied zdr.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    /// Supplied enforce distillable text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enforce_distillable_text: Option<bool>,
    /// Supplied order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    /// Supplied only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// Supplied ignore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    /// Supplied quantizations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<String>>,
    /// Supplied sort.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json", deserialize_with = "present_value")]
    pub sort: Option<serde_json::Value>,
    /// Supplied max price.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json")]
    pub max_price: Option<serde_json::Map<String, serde_json::Value>>,
    /// Supplied preferred min throughput.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json", deserialize_with = "present_value")]
    pub preferred_min_throughput: Option<serde_json::Value>,
    /// Supplied preferred max latency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json", deserialize_with = "present_value")]
    pub preferred_max_latency: Option<serde_json::Value>,
}
/// VercelGatewayRouting supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VercelGatewayRouting {
    /// Supplied only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    /// Supplied order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
}
/// Model supplied record.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Supplied id.
    pub id: String,
    /// Supplied name.
    pub name: String,
    /// Supplied api.
    pub api: Api,
    /// Supplied provider.
    pub provider: Provider,
    /// Supplied base url.
    pub base_url: String,
    /// Supplied reasoning.
    pub reasoning: bool,
    /// Supplied thinking level map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json")]
    pub thinking_level_map: Option<ThinkingLevelMap>,
    /// Supplied input.
    pub input: Vec<String>,
    /// Supplied cost.
    pub cost: TokenRates,
    /// Supplied context window.
    pub context_window: f64,
    /// Supplied max tokens.
    pub max_tokens: f64,
    /// Supplied headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json")]
    pub headers: Option<serde_json::Map<String, serde_json::Value>>,
    /// Supplied compat.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json", deserialize_with = "present_value")]
    pub compat: Option<serde_json::Value>,
}

fn present_value<'de, D: serde::Deserializer<'de>, T: serde::Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
mod locked {
    use super::*;
    pub fn serialize<T: serde::Serialize, S: serde::Serializer>(
        value: &Arc<RwLock<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        value.read().unwrap_or_else(|p| p.into_inner()).serialize(s)
    }
    pub fn deserialize<'de, T: serde::Deserialize<'de>, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Arc<RwLock<T>>, D::Error> {
        T::deserialize(d).map(|v| Arc::new(RwLock::new(v)))
    }
}
impl PartialEq for AssistantContent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Text(a), Self::Text(b)) => a == b,
            (Self::Thinking(a), Self::Thinking(b)) => a == b,
            (Self::ToolCall(a), Self::ToolCall(b)) => {
                let a = a.read().unwrap_or_else(|p| p.into_inner()).clone();
                let b = b.read().unwrap_or_else(|p| p.into_inner()).clone();
                a == b
            }
            _ => false,
        }
    }
}
impl PartialEq for AssistantMessageEvent {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Start { partial: a_partial }, Self::Start { partial: b_partial }) => {
                let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                a == b
            }
            (
                Self::TextStart {
                    content_index: a_content_index,
                    partial: a_partial,
                },
                Self::TextStart {
                    content_index: b_content_index,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::TextDelta {
                    content_index: a_content_index,
                    delta: a_delta,
                    partial: a_partial,
                },
                Self::TextDelta {
                    content_index: b_content_index,
                    delta: b_delta,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && a_delta == b_delta
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::TextEnd {
                    content_index: a_content_index,
                    content: a_content,
                    partial: a_partial,
                },
                Self::TextEnd {
                    content_index: b_content_index,
                    content: b_content,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && a_content == b_content
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ThinkingStart {
                    content_index: a_content_index,
                    partial: a_partial,
                },
                Self::ThinkingStart {
                    content_index: b_content_index,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ThinkingDelta {
                    content_index: a_content_index,
                    delta: a_delta,
                    partial: a_partial,
                },
                Self::ThinkingDelta {
                    content_index: b_content_index,
                    delta: b_delta,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && a_delta == b_delta
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ThinkingEnd {
                    content_index: a_content_index,
                    content: a_content,
                    partial: a_partial,
                },
                Self::ThinkingEnd {
                    content_index: b_content_index,
                    content: b_content,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && a_content == b_content
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ToolcallStart {
                    content_index: a_content_index,
                    partial: a_partial,
                },
                Self::ToolcallStart {
                    content_index: b_content_index,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ToolcallDelta {
                    content_index: a_content_index,
                    delta: a_delta,
                    partial: a_partial,
                },
                Self::ToolcallDelta {
                    content_index: b_content_index,
                    delta: b_delta,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && a_delta == b_delta
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::ToolcallEnd {
                    content_index: a_content_index,
                    tool_call: a_tool_call,
                    partial: a_partial,
                },
                Self::ToolcallEnd {
                    content_index: b_content_index,
                    tool_call: b_tool_call,
                    partial: b_partial,
                },
            ) => {
                a_content_index == b_content_index
                    && ({
                        let a = a_tool_call
                            .read()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        let b = b_tool_call
                            .read()
                            .unwrap_or_else(|p| p.into_inner())
                            .clone();
                        a == b
                    })
                    && ({
                        let a = a_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_partial.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::Done {
                    reason: a_reason,
                    message: a_message,
                },
                Self::Done {
                    reason: b_reason,
                    message: b_message,
                },
            ) => {
                a_reason == b_reason
                    && ({
                        let a = a_message.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_message.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            (
                Self::Error {
                    reason: a_reason,
                    error: a_error,
                },
                Self::Error {
                    reason: b_reason,
                    error: b_error,
                },
            ) => {
                a_reason == b_reason
                    && ({
                        let a = a_error.read().unwrap_or_else(|p| p.into_inner()).clone();
                        let b = b_error.read().unwrap_or_else(|p| p.into_inner()).clone();
                        a == b
                    })
            }
            _ => false,
        }
    }
}

fn serialize_map<S: serde::Serializer>(
    map: &serde_json::Map<String, serde_json::Value>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serialize_json(map, serializer)
}
pub(super) fn serialize_json<T: serde::Serialize, S: serde::Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::Serialize;
    let value = serde_json::to_value(value).map_err(serde::ser::Error::custom)?;
    ordered_json(value).serialize(serializer)
}
fn ordered_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(ordered_json).collect())
        }
        serde_json::Value::Object(map) => {
            let mut indices: Vec<_> = map
                .keys()
                .filter_map(|key| {
                    let n = key.parse::<u32>().ok()?;
                    (n != u32::MAX && n.to_string() == *key).then_some((n, key))
                })
                .collect();
            indices.sort_by_key(|(n, _)| *n);
            let mut ordered = serde_json::Map::new();
            for (_, key) in indices {
                ordered.insert(key.clone(), ordered_json(map[key].clone()));
            }
            for (key, value) in map {
                if !ordered.contains_key(&key) {
                    ordered.insert(key, ordered_json(value));
                }
            }
            serde_json::Value::Object(ordered)
        }
        value => value,
    }
}

use super::{diagnostics::ThrownValue, event_stream::AssistantMessageEventStream};
use crate::Cancellation;
/// Supplied asynchronous PayloadCallback hook.
#[cfg(not(target_arch = "wasm32"))]
type PayloadCallback = Arc<
    dyn Fn(
            serde_json::Value,
            Model,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Option<serde_json::Value>, ThrownValue>>
                    + Send,
            >,
        > + Send
        + Sync,
>;
/// Supplied asynchronous PayloadCallback hook.
#[cfg(target_arch = "wasm32")]
type PayloadCallback = Arc<
    dyn Fn(
        serde_json::Value,
        Model,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<serde_json::Value>, ThrownValue>>>,
    >,
>;
/// Supplied asynchronous ResponseCallback hook.
#[cfg(not(target_arch = "wasm32"))]
type ResponseCallback = Arc<
    dyn Fn(
            ProviderResponse,
            Model,
        )
            -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ThrownValue>> + Send>>
        + Send
        + Sync,
>;
/// Supplied asynchronous ResponseCallback hook.
#[cfg(target_arch = "wasm32")]
type ResponseCallback = Arc<
    dyn Fn(
        ProviderResponse,
        Model,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ThrownValue>>>>,
>;
/// Untouched common adapter options; default means absent fields.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamOptions {
    /// Supplied temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Supplied max tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<f64>,
    /// Supplied signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<Cancellation>,
    /// Supplied api key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Supplied transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    /// Supplied cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
    /// Supplied session id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Supplied on payload.
    #[serde(skip)]
    pub on_payload: Option<PayloadCallback>,
    /// Supplied on response.
    #[serde(skip)]
    pub on_response: Option<ResponseCallback>,
    /// Supplied headers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json")]
    pub headers: Option<serde_json::Map<String, serde_json::Value>>,
    /// Supplied timeout ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<f64>,
    /// Supplied max retries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<f64>,
    /// Supplied max retry delay ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<f64>,
    /// Supplied metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(serialize_with = "serialize_json")]
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}
/// Untouched ProviderStreamOptions adapter options.
#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStreamOptions {
    /// Common options.
    #[serde(flatten)]
    pub base: StreamOptions,
    /// Supplied extra.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
impl serde::Serialize for ProviderStreamOptions {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(serde::Serialize)]
        struct Flattened<'a> {
            #[serde(flatten)]
            base: &'a StreamOptions,
            #[serde(flatten)]
            extra: &'a serde_json::Map<String, serde_json::Value>,
        }
        serialize_json(
            &Flattened {
                base: &self.base,
                extra: &self.extra,
            },
            serializer,
        )
    }
}
/// Untouched SimpleStreamOptions adapter options.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleStreamOptions {
    /// Common options.
    #[serde(flatten)]
    pub base: StreamOptions,
    /// Supplied reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ThinkingLevel>,
    /// Supplied thinking budgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_budgets: Option<ThinkingBudgets>,
}
/// Callable protocol adapter with caller-supplied options.
#[cfg(not(target_arch = "wasm32"))]
pub type StreamFunction<TOptions = StreamOptions> = Arc<
    dyn Fn(Model, Context, Option<TOptions>) -> Result<AssistantMessageEventStream, ThrownValue>
        + Send
        + Sync,
>;
/// Callable protocol adapter with caller-supplied options.
#[cfg(target_arch = "wasm32")]
pub type StreamFunction<TOptions = StreamOptions> = Arc<
    dyn Fn(Model, Context, Option<TOptions>) -> Result<AssistantMessageEventStream, ThrownValue>,
>;
