//! Cloud content-generation preparation and invocation.

use super::google::{GoogleOptions, GoogleThinking};
use super::google_shared::{
    FunctionCallingConfigMode,
    invocation::{Setup, start},
    thinking::{effort, enabled_for_effort},
};
use crate::{
    AssistantMessageEventStream, Context, Local, Model, ProviderExecution, ProviderInvocation,
    ProviderSpawner, SimpleStreamOptions, StreamOptions, StreamOptionsFamily, Threaded,
    build_base_options_for,
};

mod policy;
#[cfg(test)]
mod tests;

/// Cloud controls with independently selected authentication and resource scope.
#[derive(Clone, Default)]
pub struct GoogleVertexOptions<E: ProviderExecution = Threaded> {
    /// Common stream controls.
    pub common: StreamOptions<E>,
    /// Tool mode when the context has tools.
    pub tool_choice: Option<FunctionCallingConfigMode>,
    /// Raw thinking request.
    pub thinking: Option<GoogleThinking>,
    /// Explicit native project before environment fallback.
    pub project: Option<String>,
    /// Explicit native location before environment fallback.
    pub location: Option<String>,
}

impl StreamOptionsFamily for GoogleVertexOptions {
    type Options<E: ProviderExecution> = GoogleVertexOptions<E>;
}

/// Start raw cloud content generation, settling setup failures into the stream.
#[must_use]
pub fn stream_google_vertex(
    model: Model,
    context: Context,
    options: Option<GoogleVertexOptions>,
) -> AssistantMessageEventStream {
    dispatch(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start raw cloud generation using caller-owned admission and hook captures.
#[must_use]
pub fn stream_google_vertex_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<GoogleVertexOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch(model, context, E::submission(options, E::provided(submit)))
}

/// Start simple cloud generation with cloud-family thinking policy.
#[must_use]
pub fn stream_simple_google_vertex(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    dispatch_simple(model, context, ProviderInvocation::Threaded(options, None))
}

/// Start simple cloud generation using caller-owned admission and hook captures.
#[must_use]
pub fn stream_simple_google_vertex_for<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<SimpleStreamOptions<E>>,
    submit: ProviderSpawner<E>,
) -> AssistantMessageEventStream {
    dispatch_simple(model, context, E::submission(options, E::provided(submit)))
}

/// Select concrete future bounds for the shared producer.
pub(crate) fn dispatch(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<GoogleVertexOptions>,
) -> AssistantMessageEventStream {
    match invocation {
        ProviderInvocation::Local(options, submit) => {
            start::<Local>(setup(model, context, options), submit)
        }
        ProviderInvocation::Threaded(options, submit) => {
            start::<Threaded>(setup(model, context, options), submit)
        }
    }
}

/// Independent cloud-provider generated identifier sequence.
static TOOL_CALL_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Retain selected settings without borrowing across the hook.
fn setup<E: ProviderExecution>(
    model: Model,
    context: Context,
    options: Option<GoogleVertexOptions<E>>,
) -> Setup<E> {
    let options = options.unwrap_or_default();
    let client = policy::client(&model, &options);
    Setup {
        model,
        context,
        client,
        options: GoogleOptions {
            common: options.common,
            tool_choice: options.tool_choice,
            thinking: options.thinking,
        },
        cloud_thinking: true,
        counter: &TOOL_CALL_COUNTER,
    }
}

/// Adapt simple common controls without forwarding raw cloud scope extras.
fn simple<E: ProviderExecution>(
    model: &Model,
    options: Option<&SimpleStreamOptions<E>>,
) -> GoogleVertexOptions<E> {
    let thinking = options
        .as_ref()
        .and_then(|options| options.reasoning)
        .map_or_else(GoogleThinking::default, |reasoning| {
            enabled_for_effort(
                model,
                effort(model, reasoning),
                options
                    .as_ref()
                    .and_then(|options| options.thinking_budgets.as_ref()),
                true,
            )
        });
    GoogleVertexOptions {
        common: build_base_options_for(model, options, None),
        thinking: Some(thinking),
        ..Default::default()
    }
}

/// Resolve cloud simple controls within the original execution family.
pub(crate) fn dispatch_simple(
    model: Model,
    context: Context,
    invocation: ProviderInvocation<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    let invocation = match invocation {
        ProviderInvocation::Local(options, submit) => {
            Local::submission(Some(simple(&model, options.as_ref())), submit)
        }
        ProviderInvocation::Threaded(options, submit) => {
            Threaded::submission(Some(simple(&model, options.as_ref())), submit)
        }
    };
    dispatch(model, context, invocation)
}
