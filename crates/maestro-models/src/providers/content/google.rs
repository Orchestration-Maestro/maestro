//! Public content-generation preparation and independently produced invocation.

use std::borrow::Cow;

use super::google_shared::{
    ApiClient, FunctionCallingConfigMode, GenerateContentParameters, GoogleThinkingLevel,
    ThinkingConfig, build_parameters, enabled_thinking, is_gemini_3_flash, is_gemini_3_pro,
    is_gemma_4,
};
use crate::options::resolve_api_key;
use crate::{
    Context, DiagnosticErrorInfo, Local, Model, ModelThinkingLevel, ProviderExecution,
    ProviderInvocation, ProviderSpawner, SimpleStreamOptions, StreamOptions, StreamOptionsFamily,
    ThinkingBudgets, ThinkingLevel, Threaded, build_base_options_for, clamp_reasoning,
    clamp_thinking_level,
};

/// Content-generation options: common settings, tool choice and optional raw thinking.
#[derive(Clone, Default)]
pub struct GoogleOptions<E: ProviderExecution = Threaded> {
    /// Settings shared by every provider.
    pub common: StreamOptions<E>,
    /// Tool-calling mode, used only when the context declares tools.
    pub tool_choice: Option<FunctionCallingConfigMode>,
    /// Raw thinking request; absent leaves thinking unspecified.
    pub thinking: Option<GoogleThinking>,
}

/// Raw thinking request. An explicit level wins over a budget.
#[derive(Clone, Default)]
pub struct GoogleThinking {
    /// Whether thinking is requested.
    pub enabled: bool,
    /// Token budget; -1 asks for a dynamic budget and 0 disables thinking.
    pub budget_tokens: Option<f64>,
    /// Level for models that select a level instead of a budget.
    pub level: Option<GoogleThinkingLevel>,
}

/// Prepare request parameters without resolving a key, calling hooks or sending anything.
///
/// Thinking controls are emitted only for a reasoning model: an enabled request carries its
/// level or budget with thoughts included, a disabled one selects the family's lowest setting.
///
/// # Errors
/// Fails with `Request aborted` when the supplied signal is already aborted.
pub fn build_params(
    model: &Model,
    context: &Context,
    options: Option<&GoogleOptions>,
) -> Result<GenerateContentParameters, DiagnosticErrorInfo> {
    params(model, context, options)
}

/// Assemble parameters with family-correct borrowed common controls.
fn params<E: ProviderExecution>(
    model: &Model,
    context: &Context,
    options: Option<&GoogleOptions<E>>,
) -> Result<GenerateContentParameters, DiagnosticErrorInfo> {
    let default = GoogleOptions::default();
    let options = options.unwrap_or(&default);
    let thinking = options
        .thinking
        .as_ref()
        .filter(|_| model.reasoning)
        .map(|thinking| {
            if thinking.enabled {
                enabled_thinking(thinking.level, thinking.budget_tokens)
            } else {
                disabled_thinking(&model.id)
            }
        });
    build_parameters(
        model,
        context,
        &options.common,
        options.tool_choice,
        thinking,
    )
}

/// Select raw options from simple ones: key, common settings and family thinking.
///
/// Missing reasoning requests disabled thinking. An effort is clamped to the model's supported
/// levels first; extra-high and off both select high. Level-based families map the effort to a
/// level and ignore custom budgets; other models use the selected budget slot or the family
/// default. The context and cancellation are not inspected.
///
/// # Errors
/// Fails with `No API key for provider: <provider>` when neither a nonempty supplied key nor
/// the environment provides one.
pub fn build_simple_options(
    model: &Model,
    options: Option<&SimpleStreamOptions>,
) -> Result<GoogleOptions, DiagnosticErrorInfo> {
    simple_options(model, options)
}

/// Select simple settings without changing the hook capture family.
fn simple_options<E: ProviderExecution>(
    model: &Model,
    options: Option<&SimpleStreamOptions<E>>,
) -> Result<GoogleOptions<E>, DiagnosticErrorInfo> {
    let key = resolve_api_key(
        &model.provider,
        options
            .and_then(|options| options.common.api_key.as_deref())
            .map(Cow::Borrowed),
    )
    .ok_or_else(|| {
        DiagnosticErrorInfo::error(format!("No API key for provider: {}", model.provider))
    })?;
    let thinking = match options.and_then(|options| options.reasoning) {
        None => GoogleThinking::default(),
        Some(reasoning) => {
            let budgets = options.and_then(|options| options.thinking_budgets.as_ref());
            enabled_for_effort(model, effort(model, reasoning), budgets)
        }
    };
    Ok(GoogleOptions {
        common: build_base_options_for(model, options, Some(&key)),
        tool_choice: None,
        thinking: Some(thinking),
    })
}

/// Clamp a requested level to the model's support; off and extra-high select high.
fn effort(model: &Model, requested: ThinkingLevel) -> ThinkingLevel {
    let supported = match clamp_thinking_level(model, requested.into()) {
        ModelThinkingLevel::Minimal => ThinkingLevel::Minimal,
        ModelThinkingLevel::Low => ThinkingLevel::Low,
        ModelThinkingLevel::Medium => ThinkingLevel::Medium,
        ModelThinkingLevel::Off | ModelThinkingLevel::High => ThinkingLevel::High,
        ModelThinkingLevel::Xhigh => ThinkingLevel::Xhigh,
    };
    clamp_reasoning(Some(supported)).unwrap_or(ThinkingLevel::High)
}

/// Enabled thinking for an effort: a level for level-based families, else a budget.
fn enabled_for_effort(
    model: &Model,
    effort: ThinkingLevel,
    budgets: Option<&ThinkingBudgets>,
) -> GoogleThinking {
    let level_based =
        is_gemini_3_pro(&model.id) || is_gemini_3_flash(&model.id) || is_gemma_4(&model.id);
    if level_based {
        GoogleThinking {
            enabled: true,
            budget_tokens: None,
            level: Some(thinking_level(effort, &model.id)),
        }
    } else {
        GoogleThinking {
            enabled: true,
            budget_tokens: Some(budget(&model.id, effort, budgets)),
            level: None,
        }
    }
}

/// Lowest thinking setting a family accepts, without returning thoughts.
fn disabled_thinking(model_id: &str) -> ThinkingConfig {
    let level = if is_gemini_3_pro(model_id) {
        Some(GoogleThinkingLevel::Low)
    } else if is_gemini_3_flash(model_id) || is_gemma_4(model_id) {
        Some(GoogleThinkingLevel::Minimal)
    } else {
        None
    };
    ThinkingConfig {
        include_thoughts: None,
        thinking_budget: level.is_none().then_some(0.0),
        thinking_level: level,
    }
}

/// Level for a level-based family; pro and Gemma have no middle levels.
fn thinking_level(effort: ThinkingLevel, model_id: &str) -> GoogleThinkingLevel {
    use GoogleThinkingLevel::{High, Low, Medium, Minimal};
    let low_end = matches!(effort, ThinkingLevel::Minimal | ThinkingLevel::Low);
    if is_gemini_3_pro(model_id) {
        return if low_end { Low } else { High };
    }
    if is_gemma_4(model_id) {
        return if low_end { Minimal } else { High };
    }
    match effort {
        ThinkingLevel::Minimal => Minimal,
        ThinkingLevel::Low => Low,
        ThinkingLevel::Medium => Medium,
        ThinkingLevel::High | ThinkingLevel::Xhigh => High,
    }
}

/// Selected custom budget, else the family default; unknown families use dynamic (-1).
fn budget(model_id: &str, effort: ThinkingLevel, custom: Option<&ThinkingBudgets>) -> f64 {
    let (slot, index) = match effort {
        ThinkingLevel::Minimal => (custom.and_then(|c| c.minimal), 0),
        ThinkingLevel::Low => (custom.and_then(|c| c.low), 1),
        ThinkingLevel::Medium => (custom.and_then(|c| c.medium), 2),
        ThinkingLevel::High | ThinkingLevel::Xhigh => (custom.and_then(|c| c.high), 3),
    };
    let defaults = if model_id.contains("2.5-pro") {
        [128.0, 2048.0, 8192.0, 32768.0]
    } else if model_id.contains("2.5-flash-lite") {
        [512.0, 2048.0, 8192.0, 24576.0]
    } else if model_id.contains("2.5-flash") {
        [128.0, 2048.0, 8192.0, 24576.0]
    } else {
        [-1.0; 4]
    };
    slot.unwrap_or(defaults[index])
}

/// Capture supplied client settings without validation or environment lookup.
#[must_use]
pub fn create_client(
    model: &Model,
    api_key: &str,
    options_headers: Option<&indexmap::IndexMap<String, String>>,
    default_base_url: Option<String>,
) -> ApiClient {
    let mut headers = model.headers.clone().unwrap_or_default();
    if let Some(options) = options_headers {
        headers.extend(
            options
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
    }
    let (base_url, api_version) = if model.base_url.is_empty() {
        (
            default_base_url
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_owned()),
            "v1beta".to_owned(),
        )
    } else {
        (model.base_url.clone(), String::new())
    };
    ApiClient {
        base_url,
        api_version,
        authentication: super::google_shared::api_client::Authentication::ApiKey(
            api_key.to_owned(),
        ),
        headers,
    }
}

/// Start a raw content-generation invocation on the caller's runtime.
///
/// Setup and transport failures settle into the returned event stream.
#[must_use]
pub fn stream_google(
    model: Model,
    context: Context,
    options: Option<GoogleOptions>,
) -> crate::AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start content generation using caller-owned admission and hook captures.
#[must_use]
pub fn stream_google_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<GoogleOptions<E>>,
    submit: ProviderSpawner<E>,
) -> crate::AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for GoogleOptions {
    type Options<E: ProviderExecution> = GoogleOptions<E>;
}

/// Select concrete future bounds before running one shared producer.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<GoogleOptions>,
) -> crate::AssistantMessageEventStream {
    match invocation {
        ProviderInvocation::Local(options, submit) => {
            start::<Local>(model, context, options, submit)
        }
        ProviderInvocation::Threaded(options, submit) => {
            start::<Threaded>(model, context, options, submit)
        }
    }
}

/// Future erasure local to the content-generation producer.
trait Producer: ProviderExecution {
    /// Box the already-selected worker under this family's future bounds.
    fn drive(
        invocation: Invocation,
        options: GoogleOptions<Self>,
        events: crate::AssistantMessageEventStream,
        output: crate::SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Only future auto-trait bounds differ between the concrete families.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                invocation: Invocation,
                options: GoogleOptions<Self>,
                events: crate::AssistantMessageEventStream,
                output: crate::SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(invocation, options, events, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Capture client configuration before asynchronous hooks and submit once.
fn start<E: Producer>(
    model: Model,
    context: Context,
    options: Option<GoogleOptions<E>>,
    submit: E::Submission,
) -> crate::AssistantMessageEventStream {
    use crate::providers::assistant_output::{initial_message, start_producer_for};
    use std::sync::{Arc, RwLock};
    let options = options.unwrap_or_default();
    let default_base_url = endpoint_default();
    let key = resolve_api_key(
        &model.provider,
        options.common.api_key.as_deref().map(Cow::Borrowed),
    );
    let invocation = Invocation {
        client: create_client(
            &model,
            key.as_deref().unwrap_or(""),
            options.common.headers.as_ref(),
            default_base_url,
        ),
        model: Arc::new(model),
        context,
    };
    let events = crate::AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message(&invocation.model)));
    let task = E::drive(invocation, options, events.clone(), Arc::clone(&output));
    start_producer_for::<E>(&events, &output, None, task, submit);
    events
}

/// Resolve simple settings before starting content generation.
///
/// # Errors
/// Returns a diagnostic when no nonempty API key is available.
pub fn stream_simple_google(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    dispatch_simple(model, context, ProviderInvocation::Threaded(options, None))
}

/// Resolve simple settings using caller-owned admission.
///
/// # Errors
/// Returns the existing missing-key diagnostic before producing a stream.
pub fn stream_simple_google_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    dispatch_simple(model, context, E::submission(options, E::provided(submit)))
}

/// Drive the same content-generation request under either hook family.
async fn run<E: ProviderExecution>(
    invocation: Invocation,
    options: GoogleOptions<E>,
    producer: crate::AssistantMessageEventStream,
    message: crate::SharedAssistantMessage,
) {
    if let Err(error) = invoke(invocation, &options, &producer, &message).await {
        crate::providers::assistant_output::fail(
            &producer,
            &message,
            options.common.signal.as_ref(),
            error,
        );
    }
}

/// Counter shared across independently produced invocations.
static TOOL_CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Prepare the selected payload and run its whole transport operation once.
async fn invoke<E: ProviderExecution>(
    invocation: Invocation,
    options: &GoogleOptions<E>,
    events: &crate::AssistantMessageEventStream,
    output: &crate::SharedAssistantMessage,
) -> Result<(), crate::providers::http::RequestFailure> {
    use super::google_shared::{process_google_stream, project_content_request};
    use crate::providers::assistant_output::conclude;
    use crate::providers::http::{Raced, RequestFailure, race};
    let model = invocation.model;
    let client = invocation.client;
    let params = params(&model, &invocation.context, Some(options))?;
    let payload =
        serde_json::to_value(params).map_err(|error| RequestFailure::new(error.to_string()))?;
    let mut selected = crate::PayloadHookInput {
        payload,
        signal: options.common.signal.clone(),
    };
    if let Some(hook) = &options.common.on_payload {
        selected = hook(selected, std::sync::Arc::clone(&model)).await?;
    }
    let signal = selected.signal.as_ref();
    let mut payload = selected.payload;
    let controls = payload
        .get_mut("config")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|config| config.shift_remove("httpOptions"));
    let projected = project_content_request(payload)?;
    let mut prepared = client.prepare_request(projected, controls.as_ref()).await?;
    if signal.is_some_and(crate::Cancellation::is_aborted) {
        return Err(RequestFailure::new("Request aborted"));
    }
    let timeout = prepared
        .timeout
        .and_then(|ms| std::time::Duration::try_from_secs_f64(ms / 1000.0).ok());
    let has_timeout_control = controls
        .as_ref()
        .and_then(|controls| controls.get("timeout"))
        .and_then(serde_json::Value::as_f64)
        .is_some_and(|value| value != 0.0);
    let owned_signal = (signal.is_some() || has_timeout_control).then(crate::Cancellation::new);
    prepared.request.signal.clone_from(&owned_signal);
    let fetch = options
        .common
        .fetch
        .clone()
        .unwrap_or_else(crate::default_fetch);
    let work = async {
        let body = client.dispatch(prepared.request, fetch).await?;
        events.push(crate::AssistantMessageEvent::Start {
            partial: std::sync::Arc::clone(output),
        });
        process_google_stream(body, output, events, &model, &TOOL_CALL_COUNTER).await?;
        conclude(events, output, options.common.signal.as_ref())
    };
    match race(work, timeout, signal).await {
        Raced::Done(result) => result,
        Raced::Cancelled | Raced::TimedOut => {
            if let Some(signal) = owned_signal {
                signal.abort();
            }
            Err(RequestFailure::new("Request was aborted"))
        }
    }
}

/// Owned setup captured before asynchronous payload callbacks.
struct Invocation {
    /// Model identity shared with the hook.
    model: std::sync::Arc<Model>,
    /// Conversation to prepare.
    context: Context,
    /// Captured native client settings.
    client: ApiClient,
}

/// Capture the public endpoint once, retaining the shared whitespace policy.
fn endpoint_default() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("GOOGLE_GEMINI_BASE_URL")
            .ok()
            .map(|text| {
                text.trim_matches(maestro_text::is_whitespace_scalar)
                    .to_owned()
            })
            .filter(|text| !text.is_empty())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Resolve simple content settings before selecting concrete producer bounds.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    match invocation {
        ProviderInvocation::Local(options, submit) => {
            simple::<Local>(model, context, options, submit)
        }
        ProviderInvocation::Threaded(options, submit) => {
            simple::<Threaded>(model, context, options, submit)
        }
    }
}
/// Prepare the original simple options once for either hook family.
fn simple<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    let selected = simple_options(&model, options.as_ref())?;
    drop(options);
    Ok(dispatch(
        model,
        context,
        E::submission(Some(selected), submit),
    ))
}
