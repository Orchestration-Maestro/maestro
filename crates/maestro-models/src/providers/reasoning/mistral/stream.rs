//! Independently produced conversation updates.
use super::{MistralOptions, request};
use crate::options::resolve_api_key;
use crate::providers::{
    assistant_output::{conclude_with, fail, initial_message, start_producer_for},
    http::RequestFailure,
};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Local, Model,
    ProviderExecution, ProviderInvocation, ProviderSpawner, SharedAssistantMessage,
    SimpleStreamOptions, StreamOptionsFamily, Threaded,
};
use std::borrow::Cow;
use std::sync::{Arc, RwLock};

/// Failure text of a response that finishes with the reason `error`, which the provider uses
/// for transient server failures; its "server error" wording makes the session retry it.
const ERROR_FINISH_TEXT: &str = "Provider stopped with: error (server error)";

/// Start a raw conversation stream on the caller's runtime.
///
/// Request failures are terminal error updates. Dropping the reader does not cancel work.
#[must_use]
pub fn stream_mistral(
    model: Model,
    context: Context,
    options: Option<MistralOptions>,
) -> AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start a conversation using caller-owned admission and hook captures.
#[must_use]
pub fn stream_mistral_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<MistralOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

impl StreamOptionsFamily for MistralOptions {
    type Options<E: ProviderExecution> = MistralOptions<E>;
}

/// Select concrete future bounds before entering the shared producer.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<MistralOptions>,
) -> AssistantMessageEventStream {
    invocation.dispatch(model, context, start_call::<Local>, start_call::<Threaded>)
}

/// Resolve credentials before starting a simple conversation stream.
///
/// # Errors
/// Returns a diagnostic when no nonempty credential is available.
/// Native streaming requires a running Tokio runtime.
pub fn stream_simple_mistral(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    simple::<Threaded>(model, context, options, None)
}

/// Resolve simple options with caller-owned producer admission.
///
/// # Errors
/// Returns the existing missing-key diagnostic before producing a stream.
pub fn stream_simple_mistral_for<E: ProviderExecution>(
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

/// Resolve credentials before reusing the raw producer.
fn simple<E: Producer>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: E::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let mut options = options.unwrap_or_default();
    options.common.api_key = resolve_api_key(
        model.provider.as_str(),
        options.common.api_key.take().map(Cow::Owned),
    )
    .map(Cow::into_owned);
    if options.common.api_key.is_none() {
        return Err(DiagnosticErrorInfo::new(
            None,
            format!("No API key for provider: {}", model.provider),
        ));
    }
    let (raw, effort) = request::simple_options(&model, &context, &options);
    Ok(start::<E>(model, context, raw, effort, submit))
}

/// Owned request settings retained by one producer.
struct Invocation<E: ProviderExecution> {
    /// Model shared with hooks.
    model: Arc<Model>,
    /// Request history.
    context: Context,
    /// Family-correct raw controls.
    options: MistralOptions<E>,
    /// Model-mapped reasoning effort.
    effort: Option<String>,
}

/// Concrete future erasure for the shared conversation worker.
trait Producer: ProviderExecution {
    /// Box one worker under this family's future bounds.
    fn drive(
        call: Invocation<Self>,
        stream: AssistantMessageEventStream,
        output: SharedAssistantMessage,
    ) -> Self::Future<'static, ()>;
}

/// Erase only the concrete family's future, not its invocation logic.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                call: Invocation<Self>,
                stream: AssistantMessageEventStream,
                output: SharedAssistantMessage,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(call, stream, output))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Own the producer separately from its returned reader.
fn start<E: Producer>(
    model: Model,
    context: Context,
    options: MistralOptions<E>,
    effort: Option<String>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = Arc::new(RwLock::new(initial_message(&model)));
    let call = Invocation {
        model: Arc::new(model),
        context,
        options,
        effort,
    };
    let task = E::drive(call, stream.clone(), Arc::clone(&output));
    start_producer_for::<E>(&stream, &output, None, task, submit);
    stream
}

/// Prepare, send and consume one conversation under either hook family.
async fn run<E: ProviderExecution>(
    call: Invocation<E>,
    producer: AssistantMessageEventStream,
    message: SharedAssistantMessage,
) {
    let model = call.model;
    let options = call.options;
    let result = async {
        let prepared = request::prepare(
            Arc::clone(&model),
            &call.context,
            &options,
            call.effort.as_deref(),
        )
        .await?;
        let body = request::send(prepared).await?;
        produce(&producer, &message, &model, body, &options).await
    }
    .await;
    if let Err(error) = result {
        fail(&producer, &message, options.common.signal.as_ref(), error);
    }
}

/// The failure a cancelled invocation reports.
pub(super) fn aborted() -> RequestFailure {
    RequestFailure::new("Request was aborted")
}

/// Admit the response before publishing its first update.
async fn produce<E: ProviderExecution>(
    stream: &AssistantMessageEventStream,
    output: &crate::SharedAssistantMessage,
    model: &Model,
    body: crate::HttpBody,
    options: &MistralOptions<E>,
) -> Result<(), RequestFailure> {
    stream.push(AssistantMessageEvent::Start {
        partial: Arc::clone(output),
    });
    super::sse::read(body, model, stream, output).await?;
    conclude_with(stream, output, options.common.signal.as_ref(), |_| {
        ERROR_FINISH_TEXT
    })
}

/// Pass raw controls to the existing producer without a simple-effort override.
fn start_call<E: Producer>(
    model: Model,
    context: Context,
    options: Option<MistralOptions<E>>,
    submit: E::Submission,
) -> AssistantMessageEventStream {
    start::<E>(model, context, options.unwrap_or_default(), None, submit)
}
