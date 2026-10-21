//! Owned content-generation producer shared by both endpoint policies.
use super::super::google::GoogleOptions;
use super::thinking::disabled_thinking;
use super::{ApiClient, GenerateContentParameters, build_parameters, enabled_thinking};
use crate::{
    Context, DiagnosticErrorInfo, Local, Model, ProviderExecution, ProviderInvocation,
    StreamOptionsFamily, Threaded,
};

/// Owned constructor configuration supplied to the common producer.
pub(in crate::providers::content) struct Setup<E: ProviderExecution> {
    /// Initial assistant record selected by the provider.
    pub output: crate::AssistantMessage,
    /// Invocation model.
    pub model: Model,
    /// Invocation conversation.
    pub context: Context,
    /// Common controls and raw thinking.
    pub options: GoogleOptions<E>,
    /// Constructor result captured before hooks.
    pub client: Result<ApiClient, DiagnosticErrorInfo>,
    /// Cloud thinking policy belongs to the entry, not the selected endpoint.
    pub cloud_thinking: bool,
    /// Provider-local generated identifier sequence.
    pub counter: &'static std::sync::atomic::AtomicU64,
}
/// Assemble parameters with family-correct borrowed common controls.
pub(in crate::providers::content) fn params<E: ProviderExecution>(
    model: &Model,
    context: &Context,
    options: &GoogleOptions<E>,
    cloud: bool,
) -> Result<GenerateContentParameters, DiagnosticErrorInfo> {
    let thinking = options
        .thinking
        .as_ref()
        .filter(|_| model.reasoning)
        .map(|thinking| {
            if thinking.enabled {
                enabled_thinking(thinking.level, thinking.budget_tokens)
            } else {
                disabled_thinking(&model.id, cloud)
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

/// Future erasure local to the content-generation producer.
pub(in crate::providers::content) trait Producer:
    ProviderExecution
{
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
pub(in crate::providers::content) fn start<E: Producer>(
    setup: Setup<E>,
    submit: E::Submission,
) -> crate::AssistantMessageEventStream {
    use crate::providers::assistant_output::start_producer_for;
    use std::sync::{Arc, RwLock};
    let Setup {
        output,
        model,
        context,
        options,
        client,
        counter,
        cloud_thinking,
    } = setup;
    let invocation = Invocation {
        model: Arc::new(model),
        context,
        client,
        counter,
        cloud_thinking,
    };
    let events = crate::AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(output));
    let task = E::drive(invocation, options, events.clone(), Arc::clone(&output));
    start_producer_for::<E>(&events, &output, None, task, submit);
    events
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

/// Prepare the selected payload and run its whole transport operation once.
async fn invoke<E: ProviderExecution>(
    invocation: Invocation,
    options: &GoogleOptions<E>,
    events: &crate::AssistantMessageEventStream,
    output: &crate::SharedAssistantMessage,
) -> Result<(), crate::providers::http::RequestFailure> {
    use super::process_google_stream;
    use crate::providers::assistant_output::conclude;
    use crate::providers::http::{Raced, RequestFailure, race};
    let model = invocation.model;
    let client = invocation.client?;
    let selected = selected_payload(
        &model,
        &invocation.context,
        options,
        invocation.cloud_thinking,
    )
    .await?;
    let signal = selected.signal.as_ref();
    let mut payload = selected.payload;
    let controls = payload
        .get_mut("config")
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|config| config.shift_remove("httpOptions"));
    let projected = client.project(payload)?;
    let (mut prepared, timeout) = prepare(&client, projected, controls.as_ref()).await?;
    if signal.is_some_and(crate::Cancellation::is_aborted) {
        return Err(RequestFailure::new("Request aborted"));
    }
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
        process_google_stream(body, output, events, &model, invocation.counter).await?;
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
pub(in crate::providers::content) struct Invocation {
    /// Model identity shared with the hook.
    model: std::sync::Arc<Model>,
    /// Conversation to prepare.
    context: Context,
    /// Captured native client settings.
    client: Result<ApiClient, DiagnosticErrorInfo>,
    /// Entry-specific thinking policy.
    cloud_thinking: bool,
    /// Module-owned tool-call sequence.
    counter: &'static std::sync::atomic::AtomicU64,
}

/// Authenticate without cancellation, retaining only the unspent request deadline.
async fn prepare(
    client: &ApiClient,
    projected: super::ProjectedContentRequest,
    controls: Option<&serde_json::Value>,
) -> Result<
    (
        super::api_client::PreparedContentRequest,
        Option<std::time::Duration>,
    ),
    crate::providers::http::RequestFailure,
> {
    #[cfg(not(target_arch = "wasm32"))]
    let started = tokio::time::Instant::now();
    let prepared = client.prepare_request(projected, controls).await?;
    let timeout = prepared
        .timeout
        .and_then(|ms| std::time::Duration::try_from_secs_f64(ms / 1000.0).ok());
    #[cfg(not(target_arch = "wasm32"))]
    let timeout = match timeout {
        Some(timeout) => Some(
            timeout
                .checked_sub(started.elapsed())
                .filter(|left| !left.is_zero())
                .ok_or_else(|| {
                    crate::providers::http::RequestFailure::new("Request was aborted")
                })?,
        ),
        None => None,
    };
    Ok((prepared, timeout))
}

/// Prepare parameters and await the caller hook once before projection.
async fn selected_payload<E: ProviderExecution>(
    model: &std::sync::Arc<Model>,
    context: &Context,
    options: &GoogleOptions<E>,
    cloud: bool,
) -> Result<crate::PayloadHookInput, crate::providers::http::RequestFailure> {
    use crate::providers::http::RequestFailure;
    let payload = serde_json::to_value(params(model, context, options, cloud)?)
        .map_err(|error| RequestFailure::new(error.to_string()))?;
    let selected = crate::PayloadHookInput {
        payload,
        signal: options.common.signal.clone(),
    };
    if let Some(hook) = &options.common.on_payload {
        Ok(hook(selected, std::sync::Arc::clone(model)).await?)
    } else {
        Ok(selected)
    }
}

/// Prepare the selected content family before starting its existing producer.
pub(in crate::providers::content) fn dispatch<K: StreamOptionsFamily>(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<K>,
    local: impl FnOnce(Model, Context, Option<K::Options<Local>>) -> Setup<Local>,
    threaded: impl FnOnce(Model, Context, Option<K::Options<Threaded>>) -> Setup<Threaded>,
) -> crate::AssistantMessageEventStream {
    invocation.dispatch(
        model,
        context,
        |model, context, options, submit| start::<Local>(local(model, context, options), submit),
        |model, context, options, submit| {
            start::<Threaded>(threaded(model, context, options), submit)
        },
    )
}
