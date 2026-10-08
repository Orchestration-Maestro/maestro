//! Request construction: endpoint, headers, cache policy and the typed payload.

use std::borrow::Cow;

use indexmap::IndexMap;
use serde_json::Value;

use super::OpenAICompletionsOptions;
use super::compat::{OpenAICompletionsCapability as Capability, ResolvedOpenAICompletionsCompat};
use super::messages::{
    ChatCompletionContent, ChatCompletionContentPart, ChatCompletionMessageParam,
    OpenAICompatCacheControl, convert_messages,
};
use super::payload::{
    ChatTemplateKwargs, GatewayOptions, IncludeUsage, Payload, Reasoning, Thinking, ToolParam,
};
use crate::providers::chat::cloudflare::{is_cloudflare_provider, resolve_cloudflare_base_url};
use crate::providers::chat::github_copilot_headers::{
    build_copilot_dynamic_headers, has_copilot_vision_input,
};
use crate::providers::http::RequestFailure;
use crate::{
    CacheControlFormat, CacheRetention, Context, MaxTokensField, Message, Model,
    ModelThinkingLevel, ThinkingFormat, ThinkingLevel, get_env_api_key,
};

/// Environment variable that selects long cache retention when set to `long`.
const CACHE_RETENTION_VARIABLE: &str = "MAESTRO_CACHE_RETENTION";
/// Account scope headers and the environment variables that default them.
const SCOPE_HEADERS: [(&str, &str); 2] = [
    ("openai-organization", "OPENAI_ORG_ID"),
    ("openai-project", "OPENAI_PROJECT_ID"),
];
/// Headers that carry the session identifier when the endpoint asks for affinity.
const AFFINITY_HEADERS: [&str; 3] = ["session_id", "x-client-request-id", "x-session-affinity"];
/// Endpoint appended to the base URL.
pub(super) const ENDPOINT_PATH: &str = "/chat/completions";

/// One invocation's inputs with the endpoint behavior and cache policy resolved from them.
pub(super) struct Invocation<'a> {
    /// Target model.
    model: &'a Model,
    /// Conversation to send.
    context: &'a Context,
    /// Caller options.
    options: &'a OpenAICompletionsOptions,
    /// Endpoint behavior.
    compat: ResolvedOpenAICompletionsCompat,
    /// Effective cache retention.
    retention: CacheRetention,
}

impl<'a> Invocation<'a> {
    /// Resolve compatibility and cache retention for the inputs.
    pub(super) fn new(
        model: &'a Model,
        context: &'a Context,
        options: &'a OpenAICompletionsOptions,
    ) -> Self {
        let retention = options.common.cache_retention.unwrap_or_else(|| {
            if std::env::var(CACHE_RETENTION_VARIABLE).is_ok_and(|value| value == "long") {
                CacheRetention::Long
            } else {
                CacheRetention::Short
            }
        });
        Self {
            model,
            context,
            options,
            compat: ResolvedOpenAICompletionsCompat::from(model),
            retention,
        }
    }

    /// Choose the nonempty explicit key, then the provider's environment key, then
    /// `OPENAI_API_KEY`.
    ///
    /// # Errors
    /// Fails when no key is available.
    pub(super) fn api_key(&self) -> Result<Cow<'a, str>, RequestFailure> {
        let environment = |name: &str| std::env::var(name).ok().filter(|key| !key.is_empty());
        self.options
            .common
            .api_key
            .as_deref()
            .filter(|key| !key.is_empty())
            .map(Cow::Borrowed)
            .or_else(|| get_env_api_key(&self.model.provider).map(Cow::Owned))
            .or_else(|| environment("OPENAI_API_KEY").map(Cow::Owned))
            .ok_or_else(|| {
                RequestFailure::new(
                    "OpenAI API key is required. Set OPENAI_API_KEY environment variable or pass it as an argument.",
                )
            })
    }

    /// Resolve the account placeholders of Cloudflare base URLs.
    ///
    /// # Errors
    /// Fails when a placeholder variable is unset.
    pub(super) fn base_url(&self) -> Result<Cow<'a, str>, RequestFailure> {
        if is_cloudflare_provider(&self.model.provider) {
            Ok(Cow::Owned(resolve_cloudflare_base_url(self.model)?))
        } else {
            Ok(Cow::Borrowed(&self.model.base_url))
        }
    }

    /// Session identifier for caching and affinity; none when caching is off.
    fn session_id(&self) -> Option<&str> {
        match self.retention {
            CacheRetention::None => None,
            _ => self.options.common.session_id.as_deref(),
        }
    }

    /// Layer headers from the model, the Copilot gateway, session affinity and the caller over
    /// the environment's organization and project. Names are lowercase; later layers win. The
    /// generated bearer authorization goes beneath every layer, so any layer replaces it; for
    /// `cloudflare-ai-gateway` it is sent as `cf-aig-authorization` above every layer.
    pub(super) fn headers(&self, api_key: &str) -> IndexMap<String, String> {
        let mut layered: IndexMap<String, String> = IndexMap::new();
        if let Some(headers) = &self.model.headers {
            layer(&mut layered, copied(headers));
        }
        if self.model.provider == "github-copilot" {
            let messages = &self.context.messages;
            let dynamic =
                build_copilot_dynamic_headers(messages, has_copilot_vision_input(messages));
            layer(&mut layered, dynamic);
        }
        if let Some(session) = self.session_id().filter(|id| !id.is_empty())
            && self.compat.has(Capability::SendSessionAffinityHeaders)
        {
            let affinity = AFFINITY_HEADERS.map(|name| (name.to_owned(), session.to_owned()));
            layer(&mut layered, affinity);
        }
        if let Some(headers) = &self.options.common.headers {
            layer(&mut layered, copied(headers));
        }
        let bearer = format!("Bearer {api_key}");
        let mut wire = IndexMap::from([("accept".to_owned(), "application/json".to_owned())]);
        wire.extend(scope_headers());
        if self.model.provider == "cloudflare-ai-gateway" {
            wire.extend(layered);
            wire.insert("cf-aig-authorization".to_owned(), bearer);
        } else {
            wire.insert("authorization".to_owned(), bearer);
            wire.extend(layered);
        }
        wire.insert("content-type".to_owned(), "application/json".to_owned());
        wire
    }

    /// Build the request payload as JSON.
    ///
    /// # Errors
    /// Fails when the conversation cannot be converted.
    pub(super) fn payload(&self) -> Result<Value, RequestFailure> {
        let mut messages = convert_messages(self.model, self.context, &self.compat)?;
        let mut tools = self.tools();
        if let Some(marker) = self.cache_marker() {
            apply_cache_markers(&mut messages, tools.as_deref_mut(), &marker);
        }
        let mut payload = Payload {
            model: &self.model.id,
            messages,
            stream: true,
            tool_stream: tools
                .as_ref()
                .is_some_and(|tools| !tools.is_empty())
                .then_some(true)
                .filter(|_| self.compat.has(Capability::ZaiToolStream)),
            tools,
            tool_choice: self.options.tool_choice.as_ref(),
            ..Payload::default()
        };
        self.apply_caching(&mut payload);
        self.apply_limits(&mut payload);
        self.apply_reasoning(&mut payload);
        self.apply_routing(&mut payload);
        serde_json::to_value(&payload).map_err(|error| RequestFailure::new(error.to_string()))
    }

    /// Convert tools; an empty list is still required while the history contains tool use.
    fn tools(&self) -> Option<Vec<ToolParam<'a>>> {
        let strict = self.compat.has(Capability::SupportsStrictMode);
        match self.context.tools.as_deref() {
            Some(tools) if !tools.is_empty() => Some(
                tools
                    .iter()
                    .map(|tool| ToolParam::new(tool, strict))
                    .collect(),
            ),
            _ => self
                .context
                .messages
                .iter()
                .any(|message| match message {
                    Message::ToolResult(_) => true,
                    Message::Assistant(assistant) => assistant
                        .content
                        .iter()
                        .any(|block| matches!(block, crate::AssistantContent::ToolCall(_))),
                    Message::User(_) => false,
                })
                .then(Vec::new),
        }
    }

    /// The cache marker for the endpoint's convention, if caching is on.
    fn cache_marker(&self) -> Option<OpenAICompatCacheControl> {
        if self.compat.cache_control_format != Some(CacheControlFormat::Anthropic) {
            return None;
        }
        match self.retention {
            CacheRetention::None => None,
            CacheRetention::Long if self.compat.has(Capability::SupportsLongCacheRetention) => {
                Some(OpenAICompatCacheControl {
                    ttl: Some("1h".to_owned()),
                })
            }
            CacheRetention::Short | CacheRetention::Long => {
                Some(OpenAICompatCacheControl { ttl: None })
            }
        }
    }

    /// Set the prompt-cache key and retention.
    fn apply_caching<'p>(&'p self, payload: &mut Payload<'p>) {
        let long = matches!(self.retention, CacheRetention::Long)
            && self.compat.has(Capability::SupportsLongCacheRetention);
        let direct = self.model.base_url.contains("api.openai.com")
            && !matches!(self.retention, CacheRetention::None);
        if direct || long {
            payload.prompt_cache_key = self.options.common.session_id.as_deref();
        }
        payload.prompt_cache_retention = long.then_some("24h");
    }

    /// Set usage reporting, storage, the token limit and temperature.
    fn apply_limits(&self, payload: &mut Payload<'_>) {
        let common = &self.options.common;
        if self.compat.has(Capability::SupportsUsageInStreaming) {
            payload.stream_options = Some(IncludeUsage {
                include_usage: true,
            });
        }
        if self.compat.has(Capability::SupportsStore) {
            payload.store = Some(false);
        }
        let limit = common
            .max_tokens
            .filter(|tokens| *tokens != 0.0 && !tokens.is_nan());
        match self.compat.max_tokens_field {
            MaxTokensField::MaxTokens => payload.max_tokens = limit,
            MaxTokensField::MaxCompletionTokens => payload.max_completion_tokens = limit,
        }
        payload.temperature = common.temperature;
    }

    /// Request or disable reasoning in the endpoint's convention.
    fn apply_reasoning(&self, payload: &mut Payload<'a>) {
        let model = self.model;
        if !model.reasoning {
            return;
        }
        let effort = self.options.reasoning_effort;
        let enabled = effort.is_some();
        let off = model
            .thinking_level_map
            .as_ref()
            .and_then(|map| map.get(&ModelThinkingLevel::Off));
        match self.compat.thinking_format {
            ThinkingFormat::Zai | ThinkingFormat::Qwen => payload.enable_thinking = Some(enabled),
            ThinkingFormat::QwenChatTemplate => {
                payload.chat_template_kwargs = Some(ChatTemplateKwargs {
                    enable_thinking: enabled,
                    preserve_thinking: true,
                });
            }
            ThinkingFormat::Deepseek => {
                payload.thinking = Some(Thinking {
                    r#type: if enabled { "enabled" } else { "disabled" },
                });
                payload.reasoning_effort = effort.map(|level| self.effort_name(level));
            }
            ThinkingFormat::Openrouter => {
                payload.reasoning = match (effort, off) {
                    (Some(level), _) => Some(Reasoning {
                        effort: self.effort_name(level),
                    }),
                    (None, Some(None)) => None,
                    (None, Some(Some(name))) => Some(Reasoning { effort: name }),
                    (None, None) => Some(Reasoning { effort: "none" }),
                };
            }
            ThinkingFormat::Openai if self.compat.has(Capability::SupportsReasoningEffort) => {
                payload.reasoning_effort = match effort {
                    Some(level) => Some(self.effort_name(level)),
                    None => off.and_then(Option::as_deref),
                };
            }
            ThinkingFormat::Openai => {}
        }
    }

    /// The provider's name for a reasoning level, or the level itself when unmapped.
    fn effort_name(&self, level: ThinkingLevel) -> &'a str {
        self.model
            .thinking_level_map
            .as_ref()
            .and_then(|map| map.get(&ModelThinkingLevel::from(level)))
            .and_then(Option::as_deref)
            .unwrap_or_else(|| level_name(level))
    }

    /// Forward routing preferences to the gateways that understand them.
    fn apply_routing<'p>(&'p self, payload: &mut Payload<'p>) {
        if self.model.base_url.contains("openrouter.ai") {
            payload.provider = self.compat.open_router_routing.as_ref();
        }
        if self.model.base_url.contains("ai-gateway.vercel.sh")
            && let Some(routing) = &self.compat.vercel_gateway_routing
            && (routing.only.is_some() || routing.order.is_some())
        {
            payload.provider_options = Some(GatewayOptions { gateway: routing });
        }
    }
}

/// Insert headers under lowercase names; a later insertion replaces an earlier one.
fn layer(
    layered: &mut IndexMap<String, String>,
    source: impl IntoIterator<Item = (String, String)>,
) {
    for (mut name, value) in source {
        name.make_ascii_lowercase();
        layered.insert(name, value);
    }
}

/// Copy the entries of a header layer that the model or caller owns.
fn copied(source: &IndexMap<String, String>) -> impl Iterator<Item = (String, String)> + '_ {
    source
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
}

/// Organization and project headers set from the environment; blank variables send nothing.
fn scope_headers() -> impl Iterator<Item = (String, String)> {
    SCOPE_HEADERS.into_iter().filter_map(|(header, variable)| {
        let value = std::env::var(variable).ok()?;
        let value = value.trim();
        (!value.is_empty()).then(|| (header.to_owned(), value.to_owned()))
    })
}

/// Lowercase wire name of a reasoning level.
fn level_name(level: ThinkingLevel) -> &'static str {
    match level {
        ThinkingLevel::Minimal => "minimal",
        ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High => "high",
        ThinkingLevel::Xhigh => "xhigh",
    }
}

/// Place cache markers on the first instruction, the last tool and the last conversation text.
fn apply_cache_markers(
    messages: &mut [ChatCompletionMessageParam],
    tools: Option<&mut [ToolParam<'_>]>,
    marker: &OpenAICompatCacheControl,
) {
    let instruction = messages.iter_mut().find_map(|message| match message {
        ChatCompletionMessageParam::System { content }
        | ChatCompletionMessageParam::Developer { content } => Some(content),
        _ => None,
    });
    if let Some(content) = instruction {
        mark_content(content, marker);
    }
    if let Some(tool) = tools.and_then(<[ToolParam<'_>]>::last_mut) {
        tool.cache_control = Some(marker.clone());
    }
    for message in messages.iter_mut().rev() {
        let content = match message {
            ChatCompletionMessageParam::User { content } => Some(content),
            ChatCompletionMessageParam::Assistant { content, .. } => content.as_mut(),
            _ => None,
        };
        if content.is_some_and(|content| mark_content(content, marker)) {
            return;
        }
    }
}

/// Mark the last text part of the content; report whether a part was marked.
fn mark_content(content: &mut ChatCompletionContent, marker: &OpenAICompatCacheControl) -> bool {
    match content {
        ChatCompletionContent::Text(text) if text.is_empty() => false,
        ChatCompletionContent::Text(text) => {
            let text = std::mem::take(text);
            *content = ChatCompletionContent::Parts(vec![ChatCompletionContentPart::Text(
                super::messages::ChatCompletionContentPartText {
                    text,
                    cache_control: Some(marker.clone()),
                },
            )]);
            true
        }
        ChatCompletionContent::Parts(parts) => parts
            .iter_mut()
            .rev()
            .find_map(|part| match part {
                ChatCompletionContentPart::Text(text) => Some(text),
                ChatCompletionContentPart::Image(_) => None,
            })
            .map(|text| text.cache_control = Some(marker.clone()))
            .is_some(),
    }
}
