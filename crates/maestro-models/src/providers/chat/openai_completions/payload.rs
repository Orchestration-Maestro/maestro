//! The typed chat-completion request body.

use serde::ser::Error as _;
use serde::{Serialize, Serializer};
use serde_json::Value;

use super::messages::{ChatCompletionMessageParam, OpenAICompatCacheControl};
use crate::{OpenRouterRouting, Tool, ToolChoice, VercelGatewayRouting};

/// A number written without a fraction when it is whole.
#[derive(Clone, Copy)]
pub(super) struct Number(pub(super) f64);

impl Serialize for Number {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        format!("{}", self.0)
            .parse::<serde_json::Number>()
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

/// The request body.
#[derive(Default, Serialize)]
pub(super) struct Payload {
    /// Model identifier.
    pub(super) model: String,
    /// Converted history.
    pub(super) messages: Vec<ChatCompletionMessageParam>,
    /// Always set: the response is streamed.
    pub(super) stream: bool,
    /// Session key for prompt caching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) prompt_cache_key: Option<String>,
    /// Extended cache lifetime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) prompt_cache_retention: Option<&'static str>,
    /// Ask for usage in the stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) stream_options: Option<IncludeUsage>,
    /// Opt out of server-side storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) store: Option<bool>,
    /// Output limit for endpoints that call it `max_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_tokens: Option<Number>,
    /// Output limit for endpoints that call it `max_completion_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_completion_tokens: Option<Number>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temperature: Option<Number>,
    /// Declared tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tools: Option<Vec<ToolParam>>,
    /// Stream tool arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_stream: Option<bool>,
    /// Forced tool selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_choice: Option<ToolChoice>,
    /// Reasoning switch for Z.ai and Qwen endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) enable_thinking: Option<bool>,
    /// Reasoning switch passed to chat templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) chat_template_kwargs: Option<ChatTemplateKwargs>,
    /// Reasoning switch for `DeepSeek` endpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) thinking: Option<Thinking>,
    /// Reasoning effort.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning_effort: Option<String>,
    /// Reasoning object for `OpenRouter`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning: Option<Reasoning>,
    /// `OpenRouter` routing preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) provider: Option<OpenRouterRouting>,
    /// Gateway routing preferences.
    #[serde(rename = "providerOptions", skip_serializing_if = "Option::is_none")]
    pub(super) provider_options: Option<GatewayOptions>,
}

/// The `stream_options` object.
#[derive(Serialize)]
pub(super) struct IncludeUsage {
    /// Report usage in the final chunk.
    pub(super) include_usage: bool,
}

/// The `chat_template_kwargs` object.
#[derive(Serialize)]
pub(super) struct ChatTemplateKwargs {
    /// Whether the template enables reasoning.
    pub(super) enable_thinking: bool,
    /// Whether earlier reasoning is kept.
    pub(super) preserve_thinking: bool,
}

/// The `thinking` object.
#[derive(Serialize)]
pub(super) struct Thinking {
    /// `enabled` or `disabled`.
    pub(super) r#type: &'static str,
}

/// The `reasoning` object.
#[derive(Serialize)]
pub(super) struct Reasoning {
    /// Effort name.
    pub(super) effort: String,
}

/// The `providerOptions` object.
#[derive(Serialize)]
pub(super) struct GatewayOptions {
    /// Gateway routing.
    pub(super) gateway: VercelGatewayRouting,
}

/// A function tool declaration.
#[derive(Serialize)]
pub(super) struct ToolParam {
    /// Constant `function`.
    r#type: &'static str,
    /// The function.
    function: FunctionParam,
    /// Cache marker for the last tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_control: Option<OpenAICompatCacheControl>,
}

/// The function of a tool declaration.
#[derive(Serialize)]
struct FunctionParam {
    /// Function name.
    name: String,
    /// What the function does.
    description: String,
    /// JSON Schema of the arguments.
    parameters: Value,
    /// Loose argument checking, for endpoints that accept the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

impl ToolParam {
    /// Declare a tool; `strict` adds the loose-checking field.
    pub(super) fn new(tool: &Tool, strict: bool) -> Self {
        Self {
            r#type: "function",
            function: FunctionParam {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
                strict: strict.then_some(false),
            },
            cache_control: None,
        }
    }
}
