//! Request construction: endpoint, headers, cache policy and the typed payload.

use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;

use super::messages::{CacheControl, ToolParam, WireMessage, convert, tools};
use super::tool_names::Naming;
use super::{AnthropicOptions, AnthropicThinkingDisplay, CallOptions, ThinkingInput};
use crate::arguments::json_parse::whitespace;
use crate::options::resolve_cache_retention;
use crate::providers::chat::cloudflare::resolve_cloudflare_base_url;
use crate::providers::chat::github_copilot_headers::{
    build_copilot_dynamic_headers, has_copilot_vision_input,
};
use crate::providers::http::{
    HttpRequest, RequestFailure, copied, edge_whitespace, endpoint_url, layer,
};
use crate::providers::json_text::compact_json;
use crate::providers::model_compat::compat_flag;
use crate::{CacheRetention, Context, Model, ToolChoice, get_env_api_key};

/// Environment variable that supplies a bearer token next to the key.
const AMBIENT_TOKEN_VARIABLE: &str = "ANTHROPIC_AUTH_TOKEN";
/// Service endpoint used when the model names none.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
/// Endpoint appended to the base URL.
const ENDPOINT_PATH: &str = "/v1/messages";
/// Protocol version every request names.
const API_VERSION: &str = "2023-06-01";
/// Beta that streams tool arguments for models without eager input streaming.
const FINE_GRAINED_TOOL_STREAMING_BETA: &str = "fine-grained-tool-streaming-2025-05-14";
/// Beta that lets reasoning interleave with tool calls.
const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";
/// Failure of a request that carries neither a key nor a bearer token.
const MISSING_AUTHENTICATION_TEXT: &str = "Could not resolve authentication method. Expected either apiKey or authToken to be set. Or for one of the \"X-Api-Key\" or \"Authorization\" headers to be explicitly omitted";

/// The request body.
#[derive(Serialize)]
struct Payload<'a> {
    /// Model identifier.
    model: &'a str,
    /// Converted history.
    messages: Vec<WireMessage>,
    /// Output limit.
    max_tokens: f64,
    /// Always set: the response is streamed.
    stream: bool,
    /// Beta features consumed by the client rather than the HTTP body.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    betas: Vec<String>,
    /// System prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<Vec<SystemBlock<'a>>>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Declared tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolParam<'a>>>,
    /// Enabled, adaptive or disabled thinking.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Thinking>,
    /// Adaptive effort.
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig<'a>>,
    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata<'a>>,
    /// Forced tool selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolSelection<'a>>,
}

/// Wire thinking states carry only the members their state uses.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Thinking {
    /// Thinking explicitly disabled.
    Disabled,
    /// Adaptive thinking.
    Adaptive {
        /// Display of enabled thinking.
        display: &'static str,
    },
    /// Budget-based thinking.
    Enabled {
        /// Thinking token budget.
        budget_tokens: f64,
        /// Display of enabled thinking.
        display: &'static str,
    },
}

/// Adaptive output effort.
#[derive(Serialize)]
struct OutputConfig<'a> {
    /// Raw enum spelling or the model's mapped string.
    effort: &'a str,
}

/// The system prompt as a text block.
#[derive(Serialize)]
struct SystemBlock<'a> {
    /// Constant `text`.
    r#type: &'static str,
    /// The prompt.
    text: &'a str,
    /// Marker placed on the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

/// The `metadata` object.
#[derive(Serialize)]
struct Metadata<'a> {
    /// Caller identifier.
    user_id: &'a str,
}

/// The `tool_choice` object.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ToolSelection<'a> {
    /// The model decides.
    Auto,
    /// No tool call.
    None,
    /// Some tool call.
    Any,
    /// A call to the named tool.
    Tool {
        /// Tool name.
        name: Cow<'a, str>,
    },
}

impl<'a> ToolSelection<'a> {
    /// Apply the invocation's naming policy to a named choice.
    fn new(choice: &'a ToolChoice, naming: Naming) -> Self {
        match choice {
            ToolChoice::Auto => Self::Auto,
            ToolChoice::None => Self::None,
            ToolChoice::Required => Self::Any,
            ToolChoice::Function { name } => Self::Tool {
                name: naming.outbound(name),
            },
        }
    }
}

/// The compatibility flags of a model that shape its tool definitions.
struct ToolCompat {
    /// Whether the service streams tool arguments as they are written.
    eager_input_streaming: bool,
    /// Whether the last tool may carry a cache marker.
    cache_control: bool,
}

/// The compatibility flags of a model that shape a request, each read once with its default.
struct Compat {
    /// Flags of the tool definitions.
    tools: ToolCompat,
    /// Whether the model supports the one-hour cache lifetime.
    long_cache_retention: bool,
    /// Whether unsigned thinking is replayed with an empty signature.
    allow_empty_signature: bool,
    /// Whether the session identifier is sent as an affinity header.
    session_affinity_headers: bool,
}

impl Compat {
    /// Read the flags of `model`: tool streaming and both cache flags are on unless the model
    /// turns them off, the other two off unless it turns them on.
    fn of(model: &Model) -> Self {
        Self {
            tools: ToolCompat {
                eager_input_streaming: compat_flag(model, "supportsEagerToolInputStreaming", true),
                cache_control: compat_flag(model, "supportsCacheControlOnTools", true),
            },
            long_cache_retention: compat_flag(model, "supportsLongCacheRetention", true),
            allow_empty_signature: compat_flag(model, "allowEmptySignature", false),
            session_affinity_headers: compat_flag(model, "sendSessionAffinityHeaders", false),
        }
    }
}

/// One invocation's inputs with the cache policy resolved from them.
pub(super) struct Invocation<'a, E: crate::ProviderExecution> {
    /// Target model.
    model: &'a Model,
    /// Compatibility flags of the model.
    compat: Compat,
    /// Conversation to send.
    context: &'a Context,
    /// Caller options.
    options: &'a AnthropicOptions<E>,
    /// Thinking provenance from the simple entry.
    thinking: &'a ThinkingInput,
    /// Effective cache retention.
    retention: CacheRetention,
    /// Frozen endpoint, expanded before the payload hook.
    base_url: Cow<'a, str>,
    /// Frozen header layers, validated when sending.
    headers: IndexMap<String, String>,
    /// Whether default authentication was deliberately suppressed.
    gateway: bool,
    /// Naming selected from credentials, not overridden headers.
    pub(super) naming: Naming,
}

impl<'a, E: crate::ProviderExecution> Invocation<'a, E> {
    /// Resolve the cache retention for the inputs: the explicit option, otherwise `long` when
    /// `MAESTRO_CACHE_RETENTION` is exactly `long`, otherwise `short`.
    pub(super) fn new(
        model: &'a Model,
        context: &'a Context,
        call: &'a CallOptions<E>,
    ) -> Result<Self, RequestFailure> {
        let options = &call.raw;
        let retention = resolve_cache_retention(options.common.cache_retention);
        let mut invocation = Self {
            model,
            compat: Compat::of(model),
            context,
            options,
            retention,
            thinking: &call.thinking,
            base_url: Cow::Borrowed(&model.base_url),
            headers: IndexMap::new(),
            gateway: false,
            naming: Naming::Plain,
        };
        if options.client.is_none() {
            invocation.resolve_authorization()?;
        }
        Ok(invocation)
    }

    /// The marker the retention asks for: none for `none`, a one-hour lifetime for `long` on a
    /// model that supports it, otherwise the default lifetime.
    fn cache_control(&self) -> Option<CacheControl> {
        match self.retention {
            CacheRetention::None => None,
            CacheRetention::Long if self.compat.long_cache_retention => Some(CacheControl::LONG),
            CacheRetention::Short | CacheRetention::Long => Some(CacheControl::SHORT),
        }
    }

    /// The output limit: the caller's unless it is zero or not a number, otherwise the model's.
    fn max_tokens(&self) -> f64 {
        self.options
            .common
            .max_tokens
            .filter(|tokens| *tokens != 0.0 && !tokens.is_nan())
            .unwrap_or(self.model.max_tokens)
    }

    /// Build the request payload as JSON.
    ///
    /// # Errors
    /// Fails when the payload cannot be written as JSON.
    pub(super) fn payload(&self) -> Result<Value, RequestFailure> {
        let cache = self.cache_control();
        let common = &self.options.common;
        let (thinking, output_config) = self.thinking_fields();
        let payload = Payload {
            model: &self.model.id,
            messages: convert(
                self.model,
                self.context,
                cache,
                self.naming,
                self.compat.allow_empty_signature,
            ),
            max_tokens: self.max_tokens(),
            stream: true,
            betas: self.betas(),
            system: self.system(cache),
            temperature: common
                .temperature
                .filter(|_| self.options.thinking_enabled != Some(true)),
            tools: self
                .context
                .tools
                .as_deref()
                .filter(|declared| !declared.is_empty())
                .map(|declared| {
                    tools(
                        declared,
                        self.compat.tools.eager_input_streaming,
                        cache.filter(|_| self.compat.tools.cache_control),
                        self.naming,
                    )
                }),
            thinking,
            output_config,
            metadata: common
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user_id"))
                .and_then(Value::as_str)
                .map(|user_id| Metadata { user_id }),
            tool_choice: self
                .options
                .tool_choice
                .as_ref()
                .map(|choice| ToolSelection::new(choice, self.naming)),
        };
        serde_json::to_value(&payload).map_err(|error| RequestFailure::new(error.to_string()))
    }

    /// Select thinking only for reasoning-capable models.
    fn thinking_fields(&self) -> (Option<Thinking>, Option<OutputConfig<'_>>) {
        if !self.model.reasoning {
            return (None, None);
        }
        match self.options.thinking_enabled {
            None => return (None, None),
            Some(false) => return (Some(Thinking::Disabled), None),
            Some(true) => {}
        }
        let display = self
            .options
            .thinking_display
            .unwrap_or(AnthropicThinkingDisplay::Summarized)
            .as_str();
        if supports_adaptive_thinking(self.model) {
            let effort = match self.thinking {
                ThinkingInput::Effort(mapped) => Some(mapped.as_str()),
                ThinkingInput::Raw | ThinkingInput::Budget(_) => {
                    self.options.effort.map(super::AnthropicEffort::as_str)
                }
            }
            .filter(|effort| !effort.is_empty());
            (
                Some(Thinking::Adaptive { display }),
                effort.map(|effort| OutputConfig { effort }),
            )
        } else {
            let budget = match self.thinking {
                ThinkingInput::Budget(budget) => Some(*budget).filter(|n| !n.is_nan()),
                ThinkingInput::Raw | ThinkingInput::Effort(_) => self
                    .options
                    .thinking_budget_tokens
                    .filter(|n| *n != 0.0 && !n.is_nan()),
            }
            .unwrap_or(1024.0);
            (
                Some(Thinking::Enabled {
                    budget_tokens: budget,
                    display,
                }),
                None,
            )
        }
    }

    /// Select configured betas in layer order, otherwise the applicable protocol defaults.
    fn betas(&self) -> Vec<String> {
        let configured = self
            .model
            .headers
            .iter()
            .chain(self.options.common.headers.iter())
            .flat_map(|headers| headers.iter())
            .filter(|(name, _)| name.eq_ignore_ascii_case("anthropic-beta"))
            .map(|(_, value)| value)
            .next_back();
        if let Some(configured) = configured {
            return configured
                .split(',')
                .map(|part| part.trim_matches(whitespace))
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect::<IndexSet<_>>()
                .into_iter()
                .collect();
        }
        let mut betas = Vec::new();
        if matches!(self.naming, Naming::Subscription) {
            betas.extend(["claude-code-20250219".into(), "oauth-2025-04-20".into()]);
        }
        if self
            .context
            .tools
            .as_deref()
            .is_some_and(|tools| !tools.is_empty())
            && !self.compat.tools.eager_input_streaming
        {
            betas.push(FINE_GRAINED_TOOL_STREAMING_BETA.into());
        }
        if self.model.reasoning
            && self.options.thinking_enabled == Some(true)
            && self.options.interleaved_thinking.unwrap_or(true)
            && !supports_adaptive_thinking(self.model)
        {
            betas.push(INTERLEAVED_THINKING_BETA.into());
        }
        betas
    }

    /// Ordered protocol identity and optional caller prompt.
    fn system(&self, cache: Option<CacheControl>) -> Option<Vec<SystemBlock<'_>>> {
        let identity = matches!(self.naming, Naming::Subscription)
            .then_some("You are Claude Code, Anthropic's official CLI for Claude.");
        let prompt = self
            .context
            .system_prompt
            .as_deref()
            .filter(|text| !text.is_empty());
        let blocks: Vec<_> = identity
            .into_iter()
            .chain(prompt)
            .map(|text| SystemBlock {
                r#type: "text",
                text,
                cache_control: cache,
            })
            .collect();
        (!blocks.is_empty()).then_some(blocks)
    }

    /// Capture credentials, mode, endpoint expansion and header layers before hooks.
    fn resolve_authorization(&mut self) -> Result<(), RequestFailure> {
        let options = self.options;
        let environment_key = options
            .common
            .api_key
            .is_none()
            .then(|| get_env_api_key(&self.model.provider))
            .flatten();
        let key = options
            .common
            .api_key
            .as_deref()
            .or(environment_key.as_deref())
            .unwrap_or_default();
        self.gateway = self.model.provider == "cloudflare-ai-gateway";
        let account = self.model.provider == "github-copilot";
        let subscription = !self.gateway && !account && key.contains("sk-ant-oat");
        if subscription {
            self.naming = Naming::Subscription;
        }
        let mut headers = IndexMap::from([
            ("accept".to_owned(), "application/json".to_owned()),
            (
                "anthropic-dangerous-direct-browser-access".to_owned(),
                "true".to_owned(),
            ),
            ("anthropic-version".to_owned(), API_VERSION.to_owned()),
        ]);
        if self.gateway {
            self.base_url = Cow::Owned(
                resolve_cloudflare_base_url(self.model)
                    .map_err(|error| RequestFailure::new(error.message))?,
            );
            headers.insert("cf-aig-authorization".to_owned(), format!("Bearer {key}"));
        } else if account || subscription {
            headers.insert("authorization".to_owned(), format!("Bearer {key}"));
        } else {
            headers.insert("x-api-key".to_owned(), key.to_owned());
            if let Some(token) = ambient_token() {
                headers.insert("authorization".to_owned(), format!("Bearer {token}"));
            }
        }
        if !account && !subscription {
            self.add_session_affinity(&mut headers);
        }
        if subscription {
            headers.insert("user-agent".to_owned(), "claude-cli/2.1.75".to_owned());
            headers.insert("x-app".to_owned(), "cli".to_owned());
        }
        self.layer_model_headers(&mut headers);
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        self.headers = headers;
        Ok(())
    }

    /// Send the session identifier as an affinity header when the model's compatibility turns it
    /// on and caching is not off.
    fn add_session_affinity(&self, headers: &mut IndexMap<String, String>) {
        if self.retention != CacheRetention::None
            && self.compat.session_affinity_headers
            && let Some(session) = self
                .options
                .common
                .session_id
                .as_deref()
                .filter(|id| !id.is_empty())
        {
            headers.insert("x-session-affinity".to_owned(), session.to_owned());
        }
    }

    /// Apply model, account-history and caller layers in that order.
    fn layer_model_headers(&self, headers: &mut IndexMap<String, String>) {
        if let Some(source) = &self.model.headers {
            layer(headers, copied(source));
        }
        if self.model.provider == "github-copilot" {
            layer(
                headers,
                build_copilot_dynamic_headers(
                    &self.context.messages,
                    has_copilot_vision_input(&self.context.messages),
                ),
            );
        }
        if let Some(source) = &self.options.common.headers {
            layer(headers, copied(source));
        }
    }

    /// Require authentication unless a gateway default omission survives the header layers.
    fn headers(&self) -> Result<IndexMap<String, String>, RequestFailure> {
        let authenticated = ["x-api-key", "authorization"].iter().any(|name| {
            self.headers
                .get(*name)
                .is_some_and(|value| !value.trim_matches(edge_whitespace).is_empty())
        });
        let omitted = self.gateway
            && ["x-api-key", "authorization"]
                .iter()
                .any(|name| !self.headers.contains_key(*name));
        if !omitted && !authenticated {
            return Err(RequestFailure::new(MISSING_AUTHENTICATION_TEXT));
        }
        Ok(self.headers.clone())
    }

    /// Build the request that posts the payload to the model's endpoint.
    ///
    /// # Errors
    /// Fails when the endpoint is not a URL, headers neither authenticate nor retain a gateway
    /// omission, or the payload cannot be written as JSON.
    pub(super) fn request(&self, payload: &Value) -> Result<HttpRequest, RequestFailure> {
        let base = match self.base_url.as_ref() {
            "" => DEFAULT_BASE_URL,
            supplied => supplied,
        };
        let mut headers = self.headers()?;
        let mut payload = payload.clone();
        if let Some(betas) = payload
            .as_object_mut()
            .and_then(|body| body.shift_remove("betas"))
        {
            let betas: Vec<String> = serde_json::from_value(betas)
                .map_err(|error| RequestFailure::new(error.to_string()))?;
            layer(
                &mut headers,
                [("anthropic-beta".to_owned(), betas.join(","))],
            );
        }
        let body =
            compact_json(&payload).map_err(|error| RequestFailure::new(error.to_string()))?;
        Ok(HttpRequest {
            method: "POST".to_owned(),
            url: endpoint_url(base, ENDPOINT_PATH)?,
            headers,
            body: body.into_bytes(),
            signal: self.options.common.signal.clone(),
        })
    }
}

/// The bearer token the environment supplies, without surrounding script whitespace; an empty
/// token is no token.
fn ambient_token() -> Option<String> {
    let token = std::env::var(AMBIENT_TOKEN_VARIABLE).ok()?;
    let token = token.trim_matches(whitespace);
    (!token.is_empty()).then(|| token.to_owned())
}

/// Report whether compatibility metadata or a recognized identifier selects adaptive thinking.
pub(super) fn supports_adaptive_thinking(model: &Model) -> bool {
    if model
        .compat
        .as_ref()
        .and_then(|compat| compat.0.get("forceAdaptiveThinking"))
        == Some(&Value::Bool(true))
    {
        return true;
    }
    [
        "opus-4-6",
        "opus-4.6",
        "opus-4-7",
        "opus-4.7",
        "sonnet-4-6",
        "sonnet-4.6",
    ]
    .iter()
    .any(|marker| model.id.contains(marker))
}
