//! The typed chat-completion request body.

use serde::Serialize;
use serde_json::Value;

use super::messages::{ChatCompletionMessageParam, OpenAICompatCacheControl};
use crate::{Tool, ToolChoice};

/// The request body; the model, tools, tool choice, routing and effort names are borrowed from
/// the caller's inputs.
#[derive(Default, Serialize)]
pub(super) struct Payload<'a> {
    /// Model identifier.
    pub(super) model: &'a str,
    /// Converted history.
    pub(super) messages: Vec<ChatCompletionMessageParam>,
    /// Always set: the response is streamed.
    pub(super) stream: bool,
    /// Session key for prompt caching.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) prompt_cache_key: Option<&'a str>,
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
    pub(super) max_tokens: Option<f64>,
    /// Output limit for endpoints that call it `max_completion_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) max_completion_tokens: Option<f64>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) temperature: Option<f64>,
    /// Declared tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tools: Option<Vec<ToolParam<'a>>>,
    /// Stream tool arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_stream: Option<bool>,
    /// Forced tool selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) tool_choice: Option<&'a ToolChoice>,
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
    pub(super) reasoning_effort: Option<&'a str>,
    /// Reasoning object for `OpenRouter`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reasoning: Option<Reasoning<'a>>,
    /// `OpenRouter` routing preferences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) provider: Option<&'a Value>,
    /// Gateway routing preferences.
    #[serde(rename = "providerOptions", skip_serializing_if = "Option::is_none")]
    pub(super) provider_options: Option<GatewayOptions<'a>>,
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
pub(super) struct Reasoning<'a> {
    /// Effort name.
    pub(super) effort: &'a str,
}

/// The `providerOptions` object.
#[derive(Serialize)]
pub(super) struct GatewayOptions<'a> {
    /// Gateway routing.
    pub(super) gateway: GatewayRouting<'a>,
}

/// A function tool declaration.
#[derive(Serialize)]
pub(super) struct ToolParam<'a> {
    /// Constant `function`.
    r#type: &'static str,
    /// The function.
    function: FunctionParam<'a>,
    /// Cache marker for the last tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cache_control: Option<OpenAICompatCacheControl>,
}

/// The function of a tool declaration.
#[derive(Serialize)]
struct FunctionParam<'a> {
    /// Function name.
    name: &'a str,
    /// What the function does.
    description: &'a str,
    /// JSON Schema of the arguments.
    parameters: &'a Value,
    /// Loose argument checking, for endpoints that accept the field.
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
}

impl<'a> ToolParam<'a> {
    /// Declare a tool; `strict` adds the loose-checking field.
    pub(super) fn new(tool: &'a Tool, strict: bool) -> Self {
        Self {
            r#type: "function",
            function: FunctionParam {
                name: &tool.name,
                description: &tool.description,
                parameters: &tool.parameters,
                strict: strict.then_some(false),
            },
            cache_control: None,
        }
    }
}

/// The gateway's selected routing fields, in request order.
#[derive(Serialize)]
pub(super) struct GatewayRouting<'a> {
    /// Supplied provider allowlist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) only: Option<&'a Value>,
    /// Supplied provider preference order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) order: Option<&'a Value>,
}
