//! Public content-generation preparation and independently produced invocation.

use std::borrow::Cow;

use super::google_shared::invocation::Setup;
use super::google_shared::thinking::{effort, enabled_for_effort};
use super::google_shared::{
    ApiClient, FunctionCallingConfigMode, GenerateContentParameters, GoogleThinkingLevel,
};
use crate::options::resolve_api_key;
use crate::providers::assistant_output::initial_message_for_api;
use crate::providers::content::google_shared::invocation::dispatch as dispatch_content;
use crate::{
    Context, DiagnosticErrorInfo, Local, Model, ProviderExecution, ProviderInvocation,
    ProviderSpawner, SimpleStreamOptions, StreamOptions, StreamOptionsFamily, Threaded,
    build_base_options_for,
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
    super::google_shared::invocation::params(
        model,
        context,
        options.unwrap_or(&GoogleOptions::default()),
        false,
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
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<GoogleOptions, DiagnosticErrorInfo> {
    simple_options(model, context, options)
}

/// Select simple settings without changing the hook capture family.
fn simple_options<E: ProviderExecution>(
    model: &Model,
    context: &Context,
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
            enabled_for_effort(model, effort(model, reasoning), budgets, false)
        }
    };
    Ok(GoogleOptions {
        common: build_base_options_for(model, context, options, Some(&key)),
        tool_choice: None,
        thinking: Some(thinking),
    })
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
        resource: super::google_shared::api_client::Resource::default(),
    }
}

/// Start a raw content-generation invocation on the caller's runtime.
///
/// Setup and transport failures settle into the returned event stream.
///
/// Output uses `google-generative-ai` even if the model names another API;
/// request processing and payload hooks receive the original descriptor.
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
    dispatch_content(
        model,
        context,
        invocation,
        setup::<Local>,
        setup::<Threaded>,
    )
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

/// Resolve simple content settings before selecting concrete producer bounds.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    invocation.dispatch(model, context, simple::<Local>, simple::<Threaded>)
}
/// Prepare the original simple options once for either hook family.
fn simple<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<crate::AssistantMessageEventStream, DiagnosticErrorInfo> {
    let selected = simple_options(&model, &context, options.as_ref())?;
    drop(options);
    Ok(dispatch(
        model,
        context,
        E::submission(Some(selected), submit),
    ))
}

/// Independent public-provider tool-call sequence.
static TOOL_CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Capture ambient constructor settings before the shared producer runs.
fn setup<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<GoogleOptions<E>>,
) -> Setup<E> {
    let options = options.unwrap_or_default();
    let key = resolve_api_key(
        &model.provider,
        options.common.api_key.as_deref().map(Cow::Borrowed),
    );
    let client = super::google_shared::client_options::public(
        &model,
        key.as_deref().unwrap_or(""),
        options.common.headers.as_ref(),
    );
    Setup {
        output: initial_message_for_api(&model, "google-generative-ai"),
        model,
        context,
        options,
        client,
        cloud_thinking: false,
        counter: &TOOL_CALL_COUNTER,
    }
}
