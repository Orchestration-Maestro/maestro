//! Request construction: endpoint, headers, cache policy and the typed payload.

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;

use super::AnthropicOptions;
use super::messages::{CacheControl, ToolParam, WireMessage, convert, tools};
use crate::arguments::json_parse::whitespace;
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
    system: Option<[SystemBlock<'a>; 1]>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Declared tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolParam<'a>>>,
    /// Request metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata<'a>>,
    /// Forced tool selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolSelection<'a>>,
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
        name: &'a str,
    },
}

impl<'a> From<&'a ToolChoice> for ToolSelection<'a> {
    fn from(choice: &'a ToolChoice) -> Self {
        match choice {
            ToolChoice::Auto => Self::Auto,
            ToolChoice::None => Self::None,
            ToolChoice::Required => Self::Any,
            ToolChoice::Function { name } => Self::Tool { name },
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
    /// Effective cache retention.
    retention: CacheRetention,
}

impl<'a> Invocation<'a> {
    /// Resolve the cache retention for the inputs: the explicit option, otherwise `long` when
    /// `MAESTRO_CACHE_RETENTION` is exactly `long`, otherwise `short`.
    pub(super) fn new(
        model: &'a Model,
        context: &'a Context,
        options: &'a AnthropicOptions,
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
            retention,
        }
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
        let payload = Payload {
            model: &self.model.id,
            messages: convert(self.model, self.context, cache),
            max_tokens: self.max_tokens(),
            stream: true,
            system: self
                .context
                .system_prompt
                .as_deref()
                .filter(|prompt| !prompt.is_empty())
                .map(|text| {
                    [SystemBlock {
                        r#type: "text",
                        text,
                        cache_control: cache,
                    }]
                }),
            temperature: common.temperature,
            tools: self
                .context
                .tools
                .as_deref()
                .filter(|declared| !declared.is_empty())
                .map(|declared| tools(declared, self.compat().eager_input_streaming, cache)),
            metadata: common
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("user_id"))
                .and_then(Value::as_str)
                .map(|user_id| Metadata { user_id }),
            tool_choice: self.options.tool_choice.as_ref().map(ToolSelection::from),
        };
        serde_json::to_value(&payload).map_err(|error| RequestFailure::new(error.to_string()))
    }

    /// The betas the request names: streamed tool arguments for a model without eager input
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

    /// Layer the protocol defaults, the key and ambient token, the betas, the model's headers
    /// and the caller's headers; names are lowercase and later layers replace earlier ones. The
    /// JSON encoding of the body then sets `content-type`.
    ///
    /// # Errors
    /// Fails when the layers hold neither a key nor an authorization.
    fn headers(&self) -> Result<IndexMap<String, String>, RequestFailure> {
        let key = self
            .options
            .common
            .api_key
            .clone()
            .or_else(|| get_env_api_key(&self.model.provider))
            .unwrap_or_default();
        let mut headers = IndexMap::new();
        layer(
            &mut headers,
            [
                ("accept", "application/json"),
                ("anthropic-dangerous-direct-browser-access", "true"),
                ("anthropic-version", API_VERSION),
                ("x-api-key", &key),
            ]
            .map(|(name, value)| (name.to_owned(), value.to_owned())),
        );
        if let Some(token) = ambient_token() {
            layer(
                &mut headers,
                [("authorization".to_owned(), format!("Bearer {token}"))],
            );
        }
        let betas = self.betas();
        if !betas.is_empty() {
            layer(
                &mut headers,
                [("anthropic-beta".to_owned(), betas.join(","))],
            );
        }
        for source in [&self.model.headers, &self.options.common.headers]
            .into_iter()
            .flatten()
        {
            layer(
                &mut headers,
                source
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone())),
            );
        }
        let authenticated = ["x-api-key", "authorization"].iter().any(|name| {
            headers
                .get(*name)
                .is_some_and(|value| !value.trim_matches(edge_whitespace).is_empty())
        });
        if !authenticated {
            return Err(RequestFailure::new(MISSING_AUTHENTICATION_TEXT));
        }
        layer(
            &mut headers,
            [("content-type".to_owned(), "application/json".to_owned())],
        );
        Ok(headers)
    }

    /// Build the request that posts the payload to the model's endpoint.
    ///
    /// # Errors
    /// Fails when the endpoint is not a URL, the headers hold no authentication or the payload
    /// cannot be written as JSON.
    pub(super) fn request(&self, payload: &Value) -> Result<HttpRequest, RequestFailure> {
        let base = match self.model.base_url.as_str() {
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
fn supports_adaptive_thinking(model_id: &str) -> bool {
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
