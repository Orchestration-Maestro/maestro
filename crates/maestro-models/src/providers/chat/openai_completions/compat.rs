//! Provider capability detection and the model overrides applied over it.

use std::collections::BTreeSet;

use crate::{
    CacheControlFormat, MaxTokensField, Model, ModelCompat, OpenAICompletionsCompat,
    OpenRouterRouting, ThinkingFormat, VercelGatewayRouting,
};

/// One boolean request capability of a chat-completion endpoint.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum OpenAICompletionsCapability {
    /// The endpoint accepts `store`.
    SupportsStore,
    /// The endpoint accepts the developer role for instructions.
    SupportsDeveloperRole,
    /// The endpoint accepts `reasoning_effort`.
    SupportsReasoningEffort,
    /// The endpoint reports usage when asked in the stream options.
    SupportsUsageInStreaming,
    /// Tool results must carry the tool name.
    RequiresToolResultName,
    /// An assistant message must separate tool results from the next user message.
    RequiresAssistantAfterToolResult,
    /// Replayed reasoning is sent as assistant text.
    RequiresThinkingAsText,
    /// Replayed assistant messages must carry `reasoning_content`.
    RequiresReasoningContentOnAssistantMessages,
    /// The endpoint streams tool arguments when `tool_stream` is set.
    ZaiToolStream,
    /// The endpoint accepts the `strict` tool field.
    SupportsStrictMode,
    /// Session identifiers are sent as affinity headers.
    SendSessionAffinityHeaders,
    /// The endpoint honors a one-hour cache lifetime.
    SupportsLongCacheRetention,
}

use OpenAICompletionsCapability as Capability;

/// Every capability, in declaration order.
const CAPABILITIES: [Capability; 12] = [
    Capability::SupportsStore,
    Capability::SupportsDeveloperRole,
    Capability::SupportsReasoningEffort,
    Capability::SupportsUsageInStreaming,
    Capability::RequiresToolResultName,
    Capability::RequiresAssistantAfterToolResult,
    Capability::RequiresThinkingAsText,
    Capability::RequiresReasoningContentOnAssistantMessages,
    Capability::ZaiToolStream,
    Capability::SupportsStrictMode,
    Capability::SendSessionAffinityHeaders,
    Capability::SupportsLongCacheRetention,
];

/// Fully resolved endpoint behavior: detected from the provider, then overridden by the model.
#[derive(Clone, Debug)]
pub struct ResolvedOpenAICompletionsCompat {
    /// Enabled boolean capabilities.
    capabilities: BTreeSet<OpenAICompletionsCapability>,
    /// Request field that carries the output token limit.
    pub(super) max_tokens_field: MaxTokensField,
    /// Convention used to request reasoning.
    pub(super) thinking_format: ThinkingFormat,
    /// `OpenRouter` routing the model supplied.
    pub(super) open_router_routing: Option<OpenRouterRouting>,
    /// Gateway routing the model supplied.
    pub(super) vercel_gateway_routing: Option<VercelGatewayRouting>,
    /// Prompt-cache marker convention.
    pub(super) cache_control_format: Option<CacheControlFormat>,
}

impl ResolvedOpenAICompletionsCompat {
    /// Report whether a capability is enabled.
    pub(super) fn has(&self, capability: OpenAICompletionsCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// A provider family recognized from the provider identifier or base URL.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    /// Z.ai endpoints.
    Zai,
    /// Moonshot endpoints.
    Moonshot,
    /// Cloudflare Workers AI endpoints.
    WorkersAi,
    /// Cloudflare AI Gateway endpoints.
    Gateway,
    /// Grok endpoints.
    Grok,
    /// `DeepSeek` endpoints.
    DeepSeek,
    /// Providers that reject some standard request fields.
    NonStandard,
}

/// Recognize the model's provider families.
fn detect_families(model: &Model) -> BTreeSet<Family> {
    let (provider, url) = (model.provider.as_str(), model.base_url.as_str());
    let found = |names: &[&str], hosts: &[&str]| {
        names.contains(&provider) || hosts.iter().any(|host| url.contains(host))
    };
    let mut families: BTreeSet<Family> = [
        (Family::Zai, found(&["zai"], &["api.z.ai"])),
        (
            Family::Moonshot,
            found(&["moonshotai", "moonshotai-cn"], &["api.moonshot."]),
        ),
        (
            Family::WorkersAi,
            found(&["cloudflare-workers-ai"], &["api.cloudflare.com"]),
        ),
        (
            Family::Gateway,
            found(&["cloudflare-ai-gateway"], &["gateway.ai.cloudflare.com"]),
        ),
        (Family::Grok, found(&["xai"], &["api.x.ai"])),
        (Family::DeepSeek, found(&["deepseek"], &["deepseek.com"])),
    ]
    .into_iter()
    .filter_map(|(family, present)| present.then_some(family))
    .collect();
    let rejecting = [
        Family::Zai,
        Family::Moonshot,
        Family::WorkersAi,
        Family::Gateway,
        Family::Grok,
    ];
    let non_standard = found(
        &["cerebras", "opencode"],
        &["cerebras.ai", "chutes.ai", "deepseek.com", "opencode.ai"],
    ) || rejecting.iter().any(|family| families.contains(family));
    if non_standard {
        families.insert(Family::NonStandard);
    }
    families
}

/// Whether the provider families enable a capability when the model says nothing.
fn detected(capability: Capability, families: &BTreeSet<Family>) -> bool {
    let has = |family| families.contains(&family);
    match capability {
        Capability::SupportsStore | Capability::SupportsDeveloperRole => !has(Family::NonStandard),
        Capability::SupportsReasoningEffort => {
            !(has(Family::Grok)
                || has(Family::Zai)
                || has(Family::Moonshot)
                || has(Family::Gateway))
        }
        Capability::SupportsUsageInStreaming => true,
        Capability::RequiresToolResultName
        | Capability::RequiresAssistantAfterToolResult
        | Capability::RequiresThinkingAsText
        | Capability::ZaiToolStream
        | Capability::SendSessionAffinityHeaders => false,
        Capability::RequiresReasoningContentOnAssistantMessages => has(Family::DeepSeek),
        Capability::SupportsStrictMode => !(has(Family::Moonshot) || has(Family::Gateway)),
        Capability::SupportsLongCacheRetention => !(has(Family::WorkersAi) || has(Family::Gateway)),
    }
}

/// The model's explicit setting for a capability, if it made one.
fn overridden(capability: Capability, compat: &OpenAICompletionsCompat) -> Option<bool> {
    match capability {
        Capability::SupportsStore => compat.supports_store,
        Capability::SupportsDeveloperRole => compat.supports_developer_role,
        Capability::SupportsReasoningEffort => compat.supports_reasoning_effort,
        Capability::SupportsUsageInStreaming => compat.supports_usage_in_streaming,
        Capability::RequiresToolResultName => compat.requires_tool_result_name,
        Capability::RequiresAssistantAfterToolResult => compat.requires_assistant_after_tool_result,
        Capability::RequiresThinkingAsText => compat.requires_thinking_as_text,
        Capability::RequiresReasoningContentOnAssistantMessages => {
            compat.requires_reasoning_content_on_assistant_messages
        }
        Capability::ZaiToolStream => compat.zai_tool_stream,
        Capability::SupportsStrictMode => compat.supports_strict_mode,
        Capability::SendSessionAffinityHeaders => compat.send_session_affinity_headers,
        Capability::SupportsLongCacheRetention => compat.supports_long_cache_retention,
    }
}

impl From<&Model> for ResolvedOpenAICompletionsCompat {
    fn from(model: &Model) -> Self {
        let families = detect_families(model);
        let has = |family| families.contains(&family);
        let url = model.base_url.as_str();
        let defaults = OpenAICompletionsCompat::default();
        let overrides = match &model.compat {
            Some(ModelCompat::OpenAICompletions(overrides)) => overrides.as_ref(),
            _ => &defaults,
        };
        let router = model.provider == "openrouter";
        let max_tokens_field =
            if url.contains("chutes.ai") || has(Family::Moonshot) || has(Family::Gateway) {
                MaxTokensField::MaxTokens
            } else {
                MaxTokensField::MaxCompletionTokens
            };
        let thinking_format = if has(Family::DeepSeek) {
            ThinkingFormat::Deepseek
        } else if has(Family::Zai) {
            ThinkingFormat::Zai
        } else if router || url.contains("openrouter.ai") {
            ThinkingFormat::Openrouter
        } else {
            ThinkingFormat::Openai
        };
        let cache_control_format =
            (router && model.id.starts_with("anthropic/")).then_some(CacheControlFormat::Anthropic);
        Self {
            capabilities: CAPABILITIES
                .into_iter()
                .filter(|capability| {
                    overridden(*capability, overrides)
                        .unwrap_or_else(|| detected(*capability, &families))
                })
                .collect(),
            max_tokens_field: overrides.max_tokens_field.unwrap_or(max_tokens_field),
            thinking_format: overrides.thinking_format.unwrap_or(thinking_format),
            open_router_routing: overrides.open_router_routing.clone(),
            vercel_gateway_routing: overrides.vercel_gateway_routing.clone(),
            cache_control_format: overrides.cache_control_format.or(cache_control_format),
        }
    }
}
