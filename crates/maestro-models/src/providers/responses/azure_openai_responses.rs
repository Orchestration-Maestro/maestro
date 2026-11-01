//! Cloud response deployment invocation.
use super::openai_responses::source::{self, Prepared, Source};
use super::openai_responses::{OpenAIResponsesReasoningSummary, Reasoning, diagnostic, reasoning};
use super::openai_responses_shared::{
    messages::{convert_responses_messages, convert_responses_tools},
    process_responses_stream,
};
use crate::arguments::json_parse::whitespace;
use crate::options::resolve_api_key;
use crate::providers::assistant_output::conclude;
use crate::providers::assistant_output::{fail, initial_message_for_api, start_producer_for};
use crate::providers::chat::openai_completions::scope_headers;
use crate::providers::http::{
    HttpRequest, RequestFailure, SseMessages, copied, layer, sdk_status_failure,
    send_with_status_error,
};
use crate::providers::json_text::compact_json;
use crate::providers::openai_prompt_cache::prompt_cache_key;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Local, Model,
    ProviderExecution, ProviderInvocation, ProviderSpawner, SharedAssistantMessage,
    SimpleStreamOptions, StreamOptions, StreamOptionsFamily, ThinkingLevel, Threaded,
    build_base_options_for, clamp_thinking_level, get_env_api_key,
};
use futures_util::stream;
use indexmap::IndexMap;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::Serialize;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use url::Url;

/// Common and cloud-specific response inputs.
#[derive(Clone, Default)]
pub struct AzureOpenAIResponsesOptions<E: ProviderExecution = Threaded> {
    /// Common request settings.
    pub common: StreamOptions<E>,
    /// Requested reasoning effort.
    pub reasoning_effort: Option<ThinkingLevel>,
    /// Requested reasoning summary style.
    pub reasoning_summary: Option<OpenAIResponsesReasoningSummary>,
    /// Cloud API version.
    pub azure_api_version: Option<String>,
    /// Resource shorthand.
    pub azure_resource_name: Option<String>,
    /// Authored endpoint base URL.
    pub azure_base_url: Option<String>,
    /// Request deployment name.
    pub azure_deployment_name: Option<String>,
}
/// RFC3986 query value encoding.
const VERSION_ENCODING: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
/// Read a nonempty environment value.
fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}
/// Select an endpoint and its API version without falling back after parsing fails.
fn resolve_azure_config<E: ProviderExecution>(
    model: &Model,
    options: &AzureOpenAIResponsesOptions<E>,
) -> Result<String, RequestFailure> {
    let base = options.azure_base_url.as_deref().map(|s| s.trim_matches(whitespace)).filter(|s| !s.is_empty()).map(Cow::Borrowed)
        .or_else(|| environment("AZURE_OPENAI_BASE_URL").map(|s| s.trim_matches(whitespace).to_owned()).filter(|s| !s.is_empty()).map(Cow::Owned))
        .or_else(|| options.azure_resource_name.as_deref().filter(|s| !s.is_empty()).map(Cow::Borrowed).or_else(|| environment("AZURE_OPENAI_RESOURCE_NAME").map(Cow::Owned)).map(|s| Cow::Owned(format!("https://{s}.openai.azure.com/openai/v1"))))
        .or_else(|| (!model.base_url.is_empty()).then_some(Cow::Borrowed(model.base_url.as_str())))
        .ok_or_else(|| RequestFailure::new("Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl."))?;
    let mut url = normalize_azure_base_url(&base)?;
    let version = options
        .azure_api_version
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Cow::Borrowed)
        .or_else(|| environment("AZURE_OPENAI_API_VERSION").map(Cow::Owned))
        .unwrap_or(Cow::Borrowed("v1"));
    let mut pairs: Vec<&str> = url
        .query()
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or_default();
            !percent_decode_str(key).eq(b"api-version".iter().copied())
        })
        .collect();
    let inserted = format!(
        "api-version={}",
        utf8_percent_encode(&version, VERSION_ENCODING)
    );
    pairs.push(&inserted);
    let query = pairs.join("&");
    let path = format!("{}/responses", url.path().trim_end_matches('/'));
    url.set_path(&path);
    url.set_query(Some(&query));
    url.set_fragment(None);
    Ok(url.into())
}
/// Normalize cloud root paths while leaving proxy query data intact.
fn normalize_azure_base_url(base: &str) -> Result<Url, RequestFailure> {
    let mut url = Url::parse(base.trim_matches(whitespace))
        .map_err(|_| RequestFailure::new(format!("Invalid Azure OpenAI base URL: {base}")))?;
    let path = url.path().trim_end_matches('/').to_owned();
    let cloud = url.host_str().is_some_and(|host| {
        host.ends_with(".openai.azure.com") || host.ends_with(".cognitiveservices.azure.com")
    });
    if cloud && (path.is_empty() || path == "/openai") {
        url.set_path("/openai/v1");
        url.set_query(None);
    } else {
        url.set_path(&path);
    }
    Ok(url)
}
/// Parse the first two equals fields, replacing earlier valid entries.
fn parse_deployment_name_map(value: &str) -> IndexMap<&str, &str> {
    value
        .split(',')
        .filter_map(|entry| {
            let mut fields = entry.trim_matches(whitespace).split('=');
            let (model, deployment) = (fields.next()?, fields.next()?);
            (!model.is_empty() && !deployment.is_empty()).then(|| {
                (
                    model.trim_matches(whitespace),
                    deployment.trim_matches(whitespace),
                )
            })
        })
        .collect()
}
/// Select the explicit deployment, environment mapping, or descriptor ID.
fn resolve_deployment_name<'a, E: ProviderExecution>(
    model: &'a Model,
    options: &'a AzureOpenAIResponsesOptions<E>,
) -> Cow<'a, str> {
    options
        .azure_deployment_name
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Cow::Borrowed)
        .or_else(|| {
            let value = environment("AZURE_OPENAI_DEPLOYMENT_NAME_MAP")?;
            parse_deployment_name_map(&value)
                .get(model.id.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| Cow::Owned((*s).to_owned()))
        })
        .unwrap_or(Cow::Borrowed(&model.id))
}
/// Start a cloud invocation, returning setup errors through the stream.
///
/// Output uses `azure-openai-responses`; request processing retains the supplied descriptor.
#[must_use]
pub fn stream_azure_openai_responses(
    model: Model,
    context: Context,
    options: Option<AzureOpenAIResponsesOptions>,
) -> AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start a response request with the caller's producer admission and hook family.
#[must_use]
pub fn stream_azure_openai_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<AzureOpenAIResponsesOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for AzureOpenAIResponsesOptions {
    type Options<E: ProviderExecution> = AzureOpenAIResponsesOptions<E>;
}

/// Select concrete future erasure before starting the shared producer.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<AzureOpenAIResponsesOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, produce::<Local>, produce::<Threaded>)
}

/// Box the same producer with this concrete family's future bounds.
trait Producer: ProviderExecution {
    /// Erase the retained producer without changing its work.
    fn drive(
        model: Arc<Model>,
        context: Context,
        options: AzureOpenAIResponsesOptions<Self>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only the concrete future auto-trait bounds differ.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                model: Arc<Model>,
                context: Context,
                options: AzureOpenAIResponsesOptions<Self>,
                stream: AssistantMessageEventStream,
                output: SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(model, context, options, stream, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Prepare one retained invocation and submit its producer once.
fn produce<E: Producer>(
    model: Model,
    context: Context,
    options: Option<AzureOpenAIResponsesOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message_for_api(
        &model,
        "azure-openai-responses",
    )));
    let options = options.unwrap_or_default();
    let signal = options.common.signal.clone();
    let task = E::drive(
        Arc::new(model),
        context,
        options,
        stream.clone(),
        Arc::clone(&output),
    );
    start_producer_for::<E>(&stream, &output, signal.as_ref(), task, submit);
    stream
}

/// Start a cloud invocation using simple budgets and reasoning support.
///
/// # Errors
/// Returns a missing provider key before creating a stream.
pub fn stream_simple_azure_openai_responses(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    simple::<Threaded>(model, context, options, None)
}

/// Start a simple response request with caller-owned producer admission.
///
/// # Errors
/// Returns the existing missing-key failure before creating a stream.
pub fn stream_simple_azure_openai_responses_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    dispatch_simple(model, context, E::submission(options, E::provided(submit)))
}

/// Select concrete simple preparation while preserving the original submission.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    invocation.dispatch(model, context, simple::<Local>, simple::<Threaded>)
}

/// Resolve shared simple options before starting the same producer.
fn simple<E: Producer>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let key = resolve_api_key(
        &model.provider,
        options
            .as_ref()
            .and_then(|o| o.common.api_key.as_deref())
            .map(Cow::Borrowed),
    )
    .ok_or_else(|| diagnostic(format!("No API key for provider: {}", model.provider)))?;
    let common = build_base_options_for(&model, &context, options.as_ref(), Some(&key));
    let reasoning_effort = options
        .as_ref()
        .and_then(|o| o.reasoning)
        .and_then(|level| {
            crate::reasoning_from_thinking_level(&clamp_thinking_level(&model, level.into()))
        });
    drop(key);
    drop(options);
    Ok(produce::<E>(
        model,
        context,
        Some(AzureOpenAIResponsesOptions {
            common,
            reasoning_effort,
            ..Default::default()
        }),
        submit,
    ))
}
/// Serialization-only cloud request body.
#[derive(Serialize)]
struct Payload<'a> {
    /// Resolved deployment identifier.
    model: &'a str,
    /// Converted conversation.
    input: Vec<Value>,
    /// Request streaming.
    stream: bool,
    /// Asks the service not to keep the response.
    store: bool,
    /// Supplied session key, independent of cache retention.
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<&'a str>,
    /// Nonzero, non-NaN output limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<f64>,
    /// Temperature retained when supplied.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Nonempty converted tool declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<Value>>,
    /// Reasoning selection.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning<'a>>,
    /// Encrypted replay when reasoning is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<[&'a str; 1]>,
}
/// Build protocol selections at the payload hook boundary.
fn build_params<E: ProviderExecution>(
    model: &Model,
    context: &Context,
    options: &AzureOpenAIResponsesOptions<E>,
) -> Result<Value, RequestFailure> {
    let deployment = resolve_deployment_name(model, options);
    let reasoning = reasoning(model, options.reasoning_effort, options.reasoning_summary);
    let include = reasoning
        .as_ref()
        .is_some_and(|r| r.summary.is_some())
        .then_some(["reasoning.encrypted_content"]);
    serde_json::to_value(Payload {
        model: &deployment,
        input: convert_responses_messages(
            model,
            context,
            &HashSet::from([
                "openai".to_owned(),
                "openai-codex".to_owned(),
                "opencode".to_owned(),
                "azure-openai-responses".to_owned(),
            ]),
            None,
        )?,
        stream: true,
        store: false,
        prompt_cache_key: options.common.session_id.as_deref().map(prompt_cache_key),
        max_output_tokens: options
            .common
            .max_tokens
            .filter(|n| *n != 0.0 && !n.is_nan()),
        temperature: options.common.temperature,
        tools: context
            .tools
            .as_deref()
            .filter(|tools| !tools.is_empty())
            .map(|tools| convert_responses_tools(tools, None)),
        reasoning,
        include,
    })
    .map_err(|e| RequestFailure::new(e.to_string()))
}
/// Prepare the endpoint-specific request before transport and hooks.
async fn send<E: ProviderExecution>(
    model: &Arc<Model>,
    context: &Context,
    options: &AzureOpenAIResponsesOptions<E>,
) -> Result<Prepared, RequestFailure> {
    let key = options.common.api_key.as_ref().filter(|s| !s.is_empty()).cloned().or_else(|| get_env_api_key(&model.provider)).or_else(|| environment("AZURE_OPENAI_API_KEY"))
        .ok_or_else(|| RequestFailure::new("Azure OpenAI API key is required. Set AZURE_OPENAI_API_KEY environment variable or pass it as an argument."))?;
    let url = resolve_azure_config(model, options)?;
    let mut payload = build_params(model, context, options)?;
    let mut headers = IndexMap::from([("accept".to_owned(), "application/json".to_owned())]);
    headers.extend(scope_headers());
    headers.insert("api-key".to_owned(), key);
    if let Some(authored) = &model.headers {
        layer(&mut headers, copied(authored));
    }
    if let Some(authored) = &options.common.headers {
        layer(&mut headers, copied(authored));
    }
    if let Some(hook) = &options.common.on_payload {
        payload = hook(
            crate::PayloadHookInput {
                payload,
                signal: None,
            },
            Arc::clone(model),
        )
        .await?
        .payload;
    }
    let streaming = payload.get("stream").is_some_and(source::streaming);
    headers.insert("content-type".to_owned(), "application/json".to_owned());
    let request = HttpRequest {
        method: "POST".to_owned(),
        url,
        headers,
        body: compact_json(&payload)
            .map_err(|e| RequestFailure::new(e.to_string()))?
            .into_bytes(),
        signal: options.common.signal.clone(),
    };
    let response = send_with_status_error(request, &options.common, sdk_status_failure).await?;
    let (prepared, observation) =
        source::prepare(response, streaming, options.common.signal.as_ref()).await?;
    if let Some(hook) = &options.common.on_response {
        hook(observation, Arc::clone(model)).await?;
    }
    Ok(prepared)
}
/// Reduce one invocation into its existing shared output handle.
async fn run<E: ProviderExecution>(
    model: Arc<Model>,
    context: Context,
    options: AzureOpenAIResponsesOptions<E>,
    events: AssistantMessageEventStream,
    output: SharedAssistantMessage,
) {
    let outcome = async {
        let prepared = send(&model, &context, &options).await?;
        events.push(AssistantMessageEvent::Start {
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
        process_responses_stream(Box::pin(source), &output, &events, &model, None).await?;
        conclude(&events, &output, options.common.signal.as_ref())
    }
    .await;
    if let Err(error) = outcome {
        fail(&events, &output, options.common.signal.as_ref(), error);
    }
}
