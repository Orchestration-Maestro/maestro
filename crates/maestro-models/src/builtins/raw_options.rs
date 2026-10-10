//! Decoding of the JSON extras each bundled raw protocol accepts.

use crate::providers::nullable::Nullable;
use crate::providers::{
    messages::anthropic::{AnthropicClient, AnthropicEffort, AnthropicThinkingDisplay},
    reasoning::mistral::{MistralPromptMode, MistralReasoningEffort},
    responses::openai_responses::{OpenAIResponsesReasoningSummary, OpenAIResponsesServiceTier},
};
use crate::records::types::{FunctionName, FunctionTag};
use crate::{
    AnthropicOptions, AzureOpenAIResponsesOptions, MistralOptions, MistralToolChoice,
    OpenAICompletionsOptions, OpenAIResponsesOptions, ProviderStreamOptions, ThinkingLevel,
    ToolChoice,
};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

/// Read a field that, when present, must be a `T`; an explicit null is rejected.
fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Read a field whose explicit null is distinct from omission.
fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Nullable<T>>, D::Error> {
    Nullable::deserialize(deserializer).map(Some)
}

/// Decode `extra` into the protocol's accepted fields, ignoring unknown members.
fn extras<T: serde::de::DeserializeOwned>(
    extra: serde_json::Map<String, Value>,
) -> Result<T, serde_json::Error> {
    serde_json::from_value(Value::Object(extra))
}

/// Tool choice spellings of the message protocol.
#[derive(Deserialize)]
#[serde(untagged)]
enum MessageChoice {
    /// Mode string.
    Mode(MessageMode),
    /// Named tool object.
    Named {
        /// Constant discriminator.
        #[serde(rename = "type")]
        _tag: NamedTool,
        /// Tool name.
        name: String,
    },
}

/// Mode strings of the message protocol.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum MessageMode {
    /// Let the model decide.
    Auto,
    /// Require some tool call.
    Any,
    /// Forbid tool calls.
    None,
}

/// Discriminator of a named message-protocol tool.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum NamedTool {
    /// The only accepted discriminator.
    Tool,
}

impl From<MessageChoice> for ToolChoice {
    fn from(choice: MessageChoice) -> Self {
        match choice {
            MessageChoice::Mode(MessageMode::Auto) => Self::Auto,
            MessageChoice::Mode(MessageMode::Any) => Self::Required,
            MessageChoice::Mode(MessageMode::None) => Self::None,
            MessageChoice::Named { name, .. } => Self::Function { name },
        }
    }
}

/// Raw fields of the message protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageExtras {
    /// Whether thinking is enabled.
    #[serde(default, deserialize_with = "present")]
    thinking_enabled: Option<bool>,
    /// Raw thinking budget.
    #[serde(default, deserialize_with = "present")]
    thinking_budget_tokens: Option<f64>,
    /// Adaptive effort.
    #[serde(default, deserialize_with = "present")]
    effort: Option<AnthropicEffort>,
    /// Thinking display mode.
    #[serde(default, deserialize_with = "present")]
    thinking_display: Option<AnthropicThinkingDisplay>,
    /// Whether the interleaved-reasoning beta may be requested.
    #[serde(default, deserialize_with = "present")]
    interleaved_thinking: Option<bool>,
    /// Tool selection.
    #[serde(default, deserialize_with = "present")]
    tool_choice: Option<MessageChoice>,
}

/// Decode the message protocol's options, taking the injected client from the object slot.
pub(super) fn anthropic(
    options: ProviderStreamOptions,
) -> Result<AnthropicOptions, serde_json::Error> {
    let extra: MessageExtras = extras(options.extra)?;
    Ok(AnthropicOptions {
        common: options.common,
        thinking_enabled: extra.thinking_enabled,
        thinking_budget_tokens: extra.thinking_budget_tokens,
        effort: extra.effort,
        thinking_display: extra.thinking_display,
        interleaved_thinking: extra.interleaved_thinking,
        tool_choice: extra.tool_choice.map(ToolChoice::from),
        client: options.objects.get::<AnthropicClient>().cloned(),
    })
}

/// Raw fields of the chat protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatExtras {
    /// Reasoning effort.
    #[serde(default, deserialize_with = "present")]
    reasoning_effort: Option<ThinkingLevel>,
    /// Tool selection.
    #[serde(default, deserialize_with = "present")]
    tool_choice: Option<ToolChoice>,
}

/// Decode the chat protocol's options.
pub(super) fn chat(
    options: ProviderStreamOptions,
) -> Result<OpenAICompletionsOptions, serde_json::Error> {
    let extra: ChatExtras = extras(options.extra)?;
    Ok(OpenAICompletionsOptions {
        common: options.common,
        tool_choice: extra.tool_choice,
        reasoning_effort: extra.reasoning_effort,
    })
}

/// Tool choice spellings of the conversation protocol.
#[derive(Deserialize)]
#[serde(untagged)]
enum ConversationChoice {
    /// Mode string.
    Mode(ConversationMode),
    /// Named function object.
    Function {
        /// Constant discriminator.
        #[serde(rename = "type")]
        _tag: FunctionTag,
        /// Function reference.
        function: FunctionName,
    },
}

/// Mode strings of the conversation protocol.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ConversationMode {
    /// Let the model decide.
    Auto,
    /// Forbid tool calls.
    None,
    /// Allow any tool.
    Any,
    /// Require a tool call.
    Required,
}

impl From<ConversationChoice> for MistralToolChoice {
    fn from(choice: ConversationChoice) -> Self {
        match choice {
            ConversationChoice::Mode(ConversationMode::Auto) => Self::Auto,
            ConversationChoice::Mode(ConversationMode::None) => Self::None,
            ConversationChoice::Mode(ConversationMode::Any) => Self::Any,
            ConversationChoice::Mode(ConversationMode::Required) => Self::Required,
            ConversationChoice::Function { function, .. } => Self::Function {
                name: function.name,
            },
        }
    }
}

/// Raw fields of the conversation protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationExtras {
    /// Reasoning effort.
    #[serde(default, deserialize_with = "present")]
    reasoning_effort: Option<MistralReasoningEffort>,
    /// Prompt mode.
    #[serde(default, deserialize_with = "present")]
    prompt_mode: Option<MistralPromptMode>,
    /// Tool selection.
    #[serde(default, deserialize_with = "present")]
    tool_choice: Option<ConversationChoice>,
}

/// Decode the conversation protocol's options.
pub(super) fn conversation(
    options: ProviderStreamOptions,
) -> Result<MistralOptions, serde_json::Error> {
    let extra: ConversationExtras = extras(options.extra)?;
    Ok(MistralOptions {
        common: options.common,
        tool_choice: extra.tool_choice.map(MistralToolChoice::from),
        prompt_mode: extra.prompt_mode,
        reasoning_effort: extra.reasoning_effort,
    })
}

/// Raw fields of the standard response protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResponseExtras {
    /// Reasoning effort.
    #[serde(default, deserialize_with = "present")]
    reasoning_effort: Option<ThinkingLevel>,
    /// Summary style; null and omission both mean none requested.
    #[serde(default)]
    reasoning_summary: Option<OpenAIResponsesReasoningSummary>,
    /// Service tier; null is sent and omission is not.
    #[serde(default, deserialize_with = "nullable")]
    service_tier: Option<Nullable<OpenAIResponsesServiceTier>>,
}

/// Decode the standard response protocol's options.
pub(super) fn responses(
    options: ProviderStreamOptions,
) -> Result<OpenAIResponsesOptions, serde_json::Error> {
    let extra: ResponseExtras = extras(options.extra)?;
    Ok(OpenAIResponsesOptions {
        common: options.common,
        reasoning_effort: extra.reasoning_effort,
        reasoning_summary: extra.reasoning_summary,
        service_tier: extra.service_tier.map(|tier| match tier {
            Nullable::Null => None,
            Nullable::Value(tier) => Some(tier),
        }),
    })
}

/// Raw fields of the cloud response protocol.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudExtras {
    /// Reasoning effort.
    #[serde(default, deserialize_with = "present")]
    reasoning_effort: Option<ThinkingLevel>,
    /// Summary style; null and omission both mean none requested.
    #[serde(default)]
    reasoning_summary: Option<OpenAIResponsesReasoningSummary>,
    /// API version.
    #[serde(default, deserialize_with = "present")]
    azure_api_version: Option<String>,
    /// Resource name.
    #[serde(default, deserialize_with = "present")]
    azure_resource_name: Option<String>,
    /// Base URL.
    #[serde(default, deserialize_with = "present")]
    azure_base_url: Option<String>,
    /// Deployment name.
    #[serde(default, deserialize_with = "present")]
    azure_deployment_name: Option<String>,
}

/// Decode the cloud response protocol's options.
pub(super) fn cloud_responses(
    options: ProviderStreamOptions,
) -> Result<AzureOpenAIResponsesOptions, serde_json::Error> {
    let extra: CloudExtras = extras(options.extra)?;
    Ok(AzureOpenAIResponsesOptions {
        common: options.common,
        reasoning_effort: extra.reasoning_effort,
        reasoning_summary: extra.reasoning_summary,
        azure_api_version: extra.azure_api_version,
        azure_resource_name: extra.azure_resource_name,
        azure_base_url: extra.azure_base_url,
        azure_deployment_name: extra.azure_deployment_name,
    })
}
