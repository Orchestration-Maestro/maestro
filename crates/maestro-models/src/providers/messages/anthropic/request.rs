//! Request construction: endpoint, headers, cache policy and the typed payload.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;

use super::messages::{CacheControl, ToolParam, WireMessage, convert, tools};
use super::tool_names::Naming;
use super::{AnthropicOptions, AnthropicThinkingDisplay, CallOptions, ThinkingInput};
use crate::arguments::json_parse::whitespace;
use crate::providers::chat::cloudflare::resolve_cloudflare_base_url;
use crate::providers::chat::github_copilot_headers::{
    build_copilot_dynamic_headers, has_copilot_vision_input,
};
use crate::providers::http::{HttpRequest, RequestFailure, edge_whitespace, endpoint_url};
use crate::providers::json_text::compact_json;
use crate::{CacheRetention, Context, Model, ModelCompat, ToolChoice, get_env_api_key};

/// Environment variable that selects long cache retention when set to `long`.
const CACHE_RETENTION_VARIABLE: &str = "MAESTRO_CACHE_RETENTION";
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

/// The compatibility flags of a model that shape a request.
struct Compat {
    /// Whether the service streams tool arguments as they are written.
    eager_input_streaming: bool,
    /// Whether the model supports the one-hour cache lifetime.
    long_cache_retention: bool,
}

/// One invocation's inputs with the cache policy resolved from them.
pub(super) struct Invocation<'a> {
    /// Target model.
    model: &'a Model,
    /// Conversation to send.
    context: &'a Context,
    /// Caller options.
    options: &'a AnthropicOptions,
    /// Thinking provenance from the simple entry.
    thinking: &'a ThinkingInput,
    /// Effective cache retention.
    retention: CacheRetention,
    /// Frozen endpoint, expanded before the payload hook.
    base_url: String,
    /// Frozen header layers, validated when sending.
    headers: IndexMap<String, String>,
    /// Whether default authentication was deliberately suppressed.
    gateway: bool,
    /// Naming selected from credentials, not overridden headers.
    pub(super) naming: Naming,
}

impl<'a> Invocation<'a> {
    /// Resolve the cache retention for the inputs: the explicit option, otherwise `long` when
    /// `MAESTRO_CACHE_RETENTION` is exactly `long`, otherwise `short`.
    pub(super) fn new(
        model: &'a Model,
        context: &'a Context,
        call: &'a CallOptions,
    ) -> Result<Self, RequestFailure> {
        let options = &call.raw;
        let retention = options.common.cache_retention.unwrap_or_else(|| {
            if std::env::var(CACHE_RETENTION_VARIABLE).is_ok_and(|value| value == "long") {
                CacheRetention::Long
            } else {
                CacheRetention::Short
            }
        });
        let mut invocation = Self {
            model,
            context,
            options,
            retention,
            thinking: &call.thinking,
            base_url: model.base_url.clone(),
            headers: IndexMap::new(),
            gateway: false,
            naming: Naming::Plain,
        };
        if options.client.is_none() {
            invocation.resolve_authorization()?;
        }
        Ok(invocation)
    }

    /// The compatibility flags of the model, each on unless the model turns it off.
    fn compat(&self) -> Compat {
        match &self.model.compat {
            Some(ModelCompat::AnthropicMessages(compat)) => Compat {
                eager_input_streaming: compat.supports_eager_tool_input_streaming.unwrap_or(true),
                long_cache_retention: compat.supports_long_cache_retention.unwrap_or(true),
            },
            _ => Compat {
                eager_input_streaming: true,
                long_cache_retention: true,
            },
        }
    }

    /// The marker the retention asks for: none for `none`, a one-hour lifetime for `long` on a
    /// model that supports it, otherwise the default lifetime.
    fn cache_control(&self) -> Option<CacheControl> {
        match self.retention {
            CacheRetention::None => None,
            CacheRetention::Long if self.compat().long_cache_retention => Some(CacheControl::LONG),
            CacheRetention::Short | CacheRetention::Long => Some(CacheControl::SHORT),
        }
    }

    /// The output limit: the caller's unless it is zero or not a number, otherwise a third of
    /// the model's, truncated.
    fn max_tokens(&self) -> f64 {
        self.options
            .common
            .max_tokens
            .filter(|tokens| *tokens != 0.0 && !tokens.is_nan())
            .unwrap_or_else(|| (self.model.max_tokens / 3.0).trunc())
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
            messages: convert(self.model, self.context, cache, self.naming),
            max_tokens: self.max_tokens(),
            stream: true,
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
                        self.compat().eager_input_streaming,
                        cache,
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
        if supports_adaptive_thinking(&self.model.id) {
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

    /// Optional betas: streamed tool arguments for a model without eager input
    /// streaming when tools are declared, and interleaved reasoning unless the caller turns it
    /// off or the model's reasoning is adaptive.
    fn betas(&self) -> Vec<&'static str> {
        let tools_declared = self.context.tools.as_deref().is_some_and(|t| !t.is_empty());
        let mut betas = Vec::new();
        if tools_declared && !self.compat().eager_input_streaming {
            betas.push(FINE_GRAINED_TOOL_STREAMING_BETA);
        }
        if self.options.interleaved_thinking.unwrap_or(true)
            && !supports_adaptive_thinking(&self.model.id)
        {
            betas.push(INTERLEAVED_THINKING_BETA);
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
            self.base_url = resolve_cloudflare_base_url(self.model)
                .map_err(|error| RequestFailure::new(error.message))?;
            headers.insert("cf-aig-authorization".to_owned(), format!("Bearer {key}"));
        } else if account || subscription {
            headers.insert("authorization".to_owned(), format!("Bearer {key}"));
        } else {
            headers.insert("x-api-key".to_owned(), key.to_owned());
            if let Some(token) = ambient_token() {
                headers.insert("authorization".to_owned(), format!("Bearer {token}"));
            }
        }
        let mut betas = self.betas();
        if subscription {
            betas.splice(0..0, ["claude-code-20250219", "oauth-2025-04-20"]);
            headers.insert("user-agent".to_owned(), "claude-cli/2.1.75".to_owned());
            headers.insert("x-app".to_owned(), "cli".to_owned());
        }
        if !betas.is_empty() {
            headers.insert("anthropic-beta".to_owned(), betas.join(","));
        }
        self.layer_model_headers(&mut headers);
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        self.headers = headers;
        Ok(())
    }

    /// Apply model, account-history and caller layers in that order.
    fn layer_model_headers(&self, headers: &mut IndexMap<String, String>) {
        if let Some(source) = &self.model.headers {
            layer(headers, source.iter().map(|(k, v)| (k.clone(), v.clone())));
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
            layer(headers, source.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }

    /// Validate the captured nongateway authentication only when preparing the send.
    fn headers(&self) -> Result<IndexMap<String, String>, RequestFailure> {
        let authenticated = ["x-api-key", "authorization"].iter().any(|name| {
            self.headers
                .get(*name)
                .is_some_and(|value| !value.trim_matches(edge_whitespace).is_empty())
        });
        if !self.gateway && !authenticated {
            return Err(RequestFailure::new(MISSING_AUTHENTICATION_TEXT));
        }
        Ok(self.headers.clone())
    }

    /// Build the request that posts the payload to the model's endpoint.
    ///
    /// # Errors
    /// Fails when the endpoint is not a URL, nongateway headers hold no authentication or the payload
    /// cannot be written as JSON.
    pub(super) fn request(&self, payload: &Value) -> Result<HttpRequest, RequestFailure> {
        let base = match self.base_url.as_str() {
            "" => DEFAULT_BASE_URL,
            supplied => supplied,
        };
        let headers = self.headers()?;
        let body = compact_json(payload).map_err(|error| RequestFailure::new(error.to_string()))?;
        Ok(HttpRequest {
            method: "POST".to_owned(),
            url: endpoint_url(base, ENDPOINT_PATH)?,
            headers,
            body: body.into_bytes(),
            signal: self.options.common.signal.clone(),
        })
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

/// The bearer token the environment supplies, without surrounding script whitespace; an empty
/// token is no token.
fn ambient_token() -> Option<String> {
    let token = std::env::var(AMBIENT_TOKEN_VARIABLE).ok()?;
    let token = token.trim_matches(whitespace);
    (!token.is_empty()).then(|| token.to_owned())
}

/// Report whether the model's reasoning is adaptive: Opus 4.6 and 4.7 and Sonnet 4.6, with
/// either spelling of the version.
pub(super) fn supports_adaptive_thinking(model_id: &str) -> bool {
    [
        "opus-4-6",
        "opus-4.6",
        "opus-4-7",
        "opus-4.7",
        "sonnet-4-6",
        "sonnet-4.6",
    ]
    .iter()
    .any(|marker| model_id.contains(marker))
}
