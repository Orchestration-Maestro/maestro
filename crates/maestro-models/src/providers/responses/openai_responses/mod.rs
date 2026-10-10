//! Standard response endpoint invocation.

pub(super) mod source;
use source::{Prepared, Source};

use super::openai_responses_shared::{
    OpenAIResponsesStreamOptions,
    messages::{convert_responses_messages, convert_responses_tools},
    process_responses_stream,
};
use crate::providers::assistant_output::{fail, initial_message};
use crate::providers::chat::cloudflare::{is_cloudflare_provider, resolve_cloudflare_base_url};
use crate::providers::chat::github_copilot_headers::{
    build_copilot_dynamic_headers, has_copilot_vision_input,
};
use crate::providers::chat::openai_completions::{
    copied, layer, level_name, resolve_cache_retention, scope_headers,
};
use crate::providers::http::{
    HttpRequest, RequestFailure, SseMessages, endpoint_url, sdk_status_failure,
    send_with_status_error, spawn_detached,
};
use crate::providers::json_text::compact_json;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, CacheRetention, Cancellation, Context,
    DiagnosticErrorInfo, DoneReason, Model, ModelCompat, ModelThinkingLevel,
    SharedAssistantMessage, SimpleStreamOptions, StopReason, StreamOptions, ThinkingLevel,
    build_base_options, clamp_thinking_level, get_env_api_key,
};
use futures_util::stream;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, PoisonError, RwLock};

/// Options for the standard response endpoint.
#[derive(Clone, Default)]
pub struct OpenAIResponsesOptions {
    /// Common request settings.
    pub common: StreamOptions,
    /// Requested reasoning effort.
    pub reasoning_effort: Option<ThinkingLevel>,
    /// Requested summary style for reasoning models.
    pub reasoning_summary: Option<OpenAIResponsesReasoningSummary>,
    /// Omitted, explicitly null, or a named service tier.
    pub service_tier: Option<Option<OpenAIResponsesServiceTier>>,
}

/// Summary styles admitted by the response endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAIResponsesReasoningSummary {
    /// Automatically select a summary style.
    Auto,
    /// Detailed summary.
    Detailed,
    /// Concise summary.
    Concise,
}

/// Service tiers admitted by the response endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OpenAIResponsesServiceTier {
    /// Automatically select a tier.
    Auto,
    /// Default tier.
    Default,
    /// Flexible tier.
    Flex,
    /// Scale tier.
    Scale,
    /// Priority tier.
    Priority,
}
impl OpenAIResponsesServiceTier {
    /// Wire spelling of a named tier.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Default => "default",
            Self::Flex => "flex",
            Self::Scale => "scale",
            Self::Priority => "priority",
        }
    }
}

/// Start a response invocation, reporting setup failures through its returned stream.
#[must_use]
pub fn stream_openai_responses(
    model: Model,
    context: Context,
    options: Option<OpenAIResponsesOptions>,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message(&model)));
    let options = options.unwrap_or_default();
    let signal = options.common.signal.clone();
    let task = run(
        Arc::new(model),
        context,
        options,
        stream.clone(),
        Arc::clone(&output),
    );
    if !spawn_detached(task) {
        fail(
            &stream,
            &output,
            signal.as_ref(),
            RequestFailure::new("Streaming requires a running Tokio runtime."),
        );
    }
    stream
}

/// Start a response using shared simple budgets and reasoning support.
///
/// # Errors
/// Returns a missing provider key before creating a stream.
pub fn stream_simple_openai_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let key = options
        .as_ref()
        .and_then(|o| o.common.api_key.as_deref())
        .filter(|k| !k.is_empty())
        .map(Cow::Borrowed)
        .or_else(|| get_env_api_key(&model.provider).map(Cow::Owned))
        .ok_or_else(|| diagnostic(format!("No API key for provider: {}", model.provider)))?;
    let common = build_base_options(&model, options.as_ref(), Some(&key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|o| o.reasoning)
        .and_then(|level| match clamp_thinking_level(&model, level.into()) {
            ModelThinkingLevel::Off => None,
            ModelThinkingLevel::Minimal => Some(ThinkingLevel::Minimal),
            ModelThinkingLevel::Low => Some(ThinkingLevel::Low),
            ModelThinkingLevel::Medium => Some(ThinkingLevel::Medium),
            ModelThinkingLevel::High => Some(ThinkingLevel::High),
            ModelThinkingLevel::Xhigh => Some(ThinkingLevel::Xhigh),
        });
    drop(options);
    Ok(stream_openai_responses(
        model,
        context,
        Some(OpenAIResponsesOptions {
            common,
            reasoning_effort,
            ..OpenAIResponsesOptions::default()
        }),
    ))
}

/// Failure text carried by the stream source or simple key boundary.
pub(super) fn diagnostic(message: impl Into<String>) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".to_owned()),
        message: message.into(),
        stack: None,
        code: None,
    }
}

/// Serialization-only request body.
#[derive(Serialize)]
struct Payload<'a> {
    /// Model identifier.
    model: &'a str,
    /// Converted conversation.
    input: Vec<Value>,
    /// Request a streamed response.
    stream: bool,
    /// Session key when caching is enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<&'a str>,
    /// Long retention when supported.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_retention: Option<&'a str>,
    /// Do not store the response.
    store: bool,
    /// Requested output limit; zero and NaN are omitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<f64>,
    /// Temperature retained when supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Service tier with a distinct explicit null.
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<&'a Option<OpenAIResponsesServiceTier>>,
    /// Nonempty converted tool declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    /// Reasoning request for reasoning models.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning<'a>>,
    /// Encrypted reasoning requested for replay.
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<[&'a str; 1]>,
}

/// Requested reasoning effort.
#[derive(Serialize)]
pub(super) struct Reasoning<'a> {
    /// Wire effort.
    pub(super) effort: &'a str,
    /// Summary included only for requested reasoning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) summary: Option<OpenAIResponsesReasoningSummary>,
}

/// Request inputs with their resolved cache policy.
struct Request<'a> {
    /// Target descriptor.
    model: &'a Arc<Model>,
    /// Conversation to convert.
    context: &'a Context,
    /// Endpoint options.
    options: &'a OpenAIResponsesOptions,
    /// Effective retention choice.
    retention: CacheRetention,
}
impl Request<'_> {
    /// Session key when caching is enabled, including an empty supplied key.
    fn session(&self) -> Option<&str> {
        self.options
            .common
            .session_id
            .as_deref()
            .filter(|_| self.retention != CacheRetention::None)
    }

    /// Compatibility fields supplied by this model.
    fn compat(&self) -> Option<&crate::OpenAIResponsesCompat> {
        match &self.model.compat {
            Some(ModelCompat::OpenAIResponses(compat)) => Some(compat),
            _ => None,
        }
    }

    /// Ordered case-insensitive header layers with gateway authorization applied last.
    fn headers(&self, key: &str) -> IndexMap<String, String> {
        let mut headers = IndexMap::from([("accept".to_owned(), "application/json".to_owned())]);
        headers.extend(scope_headers());
        if self.model.provider != "cloudflare-ai-gateway" {
            headers.insert("authorization".to_owned(), format!("Bearer {key}"));
        }
        if let Some(model) = &self.model.headers {
            layer(&mut headers, copied(model));
        }
        if self.model.provider == "github-copilot" {
            layer(
                &mut headers,
                build_copilot_dynamic_headers(
                    &self.context.messages,
                    has_copilot_vision_input(&self.context.messages),
                ),
            );
        }
        if let Some(session) = self.session().filter(|id| !id.is_empty()) {
            if self
                .compat()
                .and_then(|c| c.send_session_id_header)
                .unwrap_or(true)
            {
                headers.insert("session_id".to_owned(), session.to_owned());
            }
            headers.insert("x-client-request-id".to_owned(), session.to_owned());
        }
        if let Some(explicit) = &self.options.common.headers {
            layer(&mut headers, copied(explicit));
        }
        if self.model.provider == "cloudflare-ai-gateway" {
            headers.insert("cf-aig-authorization".to_owned(), format!("Bearer {key}"));
        }
        headers
    }

    /// Convert history and endpoint selections to the payload-hook boundary.
    fn payload(&self) -> Result<Value, RequestFailure> {
        let input = convert_responses_messages(
            self.model,
            self.context,
            &HashSet::from([
                "openai".to_owned(),
                "openai-codex".to_owned(),
                "opencode".to_owned(),
            ]),
            None,
        )?;
        let requested =
            self.options.reasoning_effort.is_some() || self.options.reasoning_summary.is_some();
        let reasoning = if self.model.provider == "github-copilot" && !requested {
            None
        } else {
            reasoning(
                self.model,
                self.options.reasoning_effort,
                self.options.reasoning_summary,
            )
        };
        let include = reasoning
            .as_ref()
            .is_some_and(|r| r.summary.is_some())
            .then_some(["reasoning.encrypted_content"]);
        let long = self.retention == CacheRetention::Long
            && self
                .compat()
                .and_then(|c| c.supports_long_cache_retention)
                .unwrap_or(true);
        serde_json::to_value(Payload {
            model: &self.model.id,
            input,
            tools: self
                .context
                .tools
                .as_deref()
                .filter(|tools| !tools.is_empty())
                .map(|tools| convert_responses_tools(tools, None)),
            stream: true,
            store: false,
            reasoning,
            include,
            service_tier: self.options.service_tier.as_ref(),
            temperature: self.options.common.temperature,
            max_output_tokens: self
                .options
                .common
                .max_tokens
                .filter(|n| *n != 0.0 && !n.is_nan()),
            prompt_cache_key: self.session(),
            prompt_cache_retention: long.then_some("24h"),
        })
        .map_err(|e| RequestFailure::new(e.to_string()))
    }

    /// Prepare the edited request and response before invoking the response hook.
    async fn send(&self, key: &str) -> Result<Prepared, RequestFailure> {
        let base_url = if is_cloudflare_provider(&self.model.provider) {
            Cow::Owned(resolve_cloudflare_base_url(self.model)?)
        } else {
            Cow::Borrowed(self.model.base_url.as_str())
        };
        let mut headers = self.headers(key);
        let mut payload = self.payload()?;
        if let Some(hook) = &self.options.common.on_payload {
            payload = hook(payload, Arc::clone(self.model)).await?;
        }
        if payload.is_null() {
            return Err(RequestFailure::new("Response payload must not be null"));
        }
        let streaming = payload.get("stream").is_some_and(source::streaming);
        headers.insert("content-type".to_owned(), "application/json".to_owned());
        let body = compact_json(&payload)
            .map_err(|e| RequestFailure::new(e.to_string()))?
            .into_bytes();
        let request = HttpRequest {
            method: "POST".to_owned(),
            url: endpoint_url(&base_url, "/responses")?,
            headers,
            body,
            signal: self.options.common.signal.clone(),
        };
        let response =
            send_with_status_error(request, &self.options.common, sdk_status_failure).await?;
        let (prepared, observation) =
            source::prepare(response, streaming, self.options.common.signal.as_ref()).await?;
        if let Some(hook) = &self.options.common.on_response {
            hook(observation, Arc::clone(self.model)).await?;
        }
        Ok(prepared)
    }
}

/// Select the raw key, including its last-resort standard environment key.
fn raw_key<'a>(model: &Model, options: &'a StreamOptions) -> Result<Cow<'a, str>, RequestFailure> {
    options.api_key.as_deref().filter(|key|!key.is_empty()).map(Cow::Borrowed)
        .or_else(||get_env_api_key(&model.provider).map(Cow::Owned))
        .or_else(||std::env::var("OPENAI_API_KEY").ok().filter(|key|!key.is_empty()).map(Cow::Owned))
        .ok_or_else(||RequestFailure::new("OpenAI API key is required. Set OPENAI_API_KEY environment variable or pass it as an argument."))
}

/// Run one invocation and publish its terminal update.
async fn run(
    model: Arc<Model>,
    context: Context,
    options: OpenAIResponsesOptions,
    stream: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = async {
        let key = raw_key(&model, &options.common)?;
        let request = Request {
            model: &model,
            context: &context,
            options: &options,
            retention: resolve_cache_retention(options.common.cache_retention),
        };
        let prepared = request.send(&key).await?;
        stream.push(AssistantMessageEvent::Start {
            partial: Arc::clone(&output),
        });
        let Prepared::Stream(body) = prepared else {
            return Err(RequestFailure::new("Response is not an event stream"));
        };
        let source = stream::unfold(
            Source {
                body,
                messages: SseMessages::default(),
                pending: VecDeque::new(),
                ended: false,
                done: false,
                signal: options.common.signal.clone(),
            },
            |mut source| async move { source.next().await.map(|event| (event, source)) },
        );
        let pricing = |usage: &mut crate::Usage, tier: Option<&str>| price(usage, tier, &model.id);
        let pricing_options = OpenAIResponsesStreamOptions {
            service_tier: options
                .service_tier
                .flatten()
                .map(OpenAIResponsesServiceTier::name),
            apply_service_tier_pricing: Some(&pricing),
            resolve_service_tier: None,
        };
        process_responses_stream(
            Box::pin(source),
            &output,
            &stream,
            &model,
            Some(&pricing_options),
        )
        .await?;
        conclude(&stream, &output, options.common.signal.as_ref())
    }
    .await;
    if let Err(error) = outcome {
        fail(&stream, &output, options.common.signal.as_ref(), error);
    }
}

/// Scale every cost category using the selected tier, retaining source expression order.
pub(crate) fn price(usage: &mut crate::Usage, tier: Option<&str>, model: &str) {
    let multiplier = match tier {
        Some("flex") => 0.5,
        Some("priority") if model == "gpt-5.5" => 2.5,
        Some("priority") => 2.0,
        _ => return,
    };
    usage.cost.input *= multiplier;
    usage.cost.output *= multiplier;
    usage.cost.cache_read *= multiplier;
    usage.cost.cache_write *= multiplier;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
}

/// Publish completion only after reduction and cancellation checks succeed.
pub(super) fn conclude(
    events: &AssistantMessageEventStream,
    output: &SharedAssistantMessage,
    signal: Option<&Cancellation>,
) -> Result<(), RequestFailure> {
    if signal.is_some_and(Cancellation::is_aborted) {
        return Err(RequestFailure::new("Request was aborted"));
    }
    let reason = match output
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .stop_reason
    {
        StopReason::Stop => DoneReason::Stop,
        StopReason::Length => DoneReason::Length,
        StopReason::ToolUse => DoneReason::ToolUse,
        StopReason::Error | StopReason::Aborted => {
            return Err(RequestFailure::new("An unknown error occurred"));
        }
    };
    events.push(AssistantMessageEvent::Done {
        reason,
        message: Arc::clone(output),
    });
    events.end(None);
    Ok(())
}

/// Select requested effort or the model's unrequested reasoning mapping.
pub(super) fn reasoning(
    model: &Model,
    effort: Option<ThinkingLevel>,
    summary: Option<OpenAIResponsesReasoningSummary>,
) -> Option<Reasoning<'_>> {
    if !model.reasoning {
        return None;
    }
    let map = model.thinking_level_map.as_ref();
    if effort.is_some() || summary.is_some() {
        let effort = effort.map_or("medium", |level| {
            map.and_then(|m| m.get(&level.into()))
                .and_then(Option::as_deref)
                .unwrap_or_else(|| level_name(level))
        });
        return Some(Reasoning {
            effort,
            summary: Some(summary.unwrap_or(OpenAIResponsesReasoningSummary::Auto)),
        });
    }
    let effort = match map.and_then(|m| m.get(&ModelThinkingLevel::Off)) {
        Some(None) => return None,
        Some(Some(name)) => name,
        None => "none",
    };
    Some(Reasoning {
        effort,
        summary: None,
    })
}
