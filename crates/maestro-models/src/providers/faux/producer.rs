//! Shared scripted production with family-correct admission and future erasure.
use super::{
    FauxAssistantMessageOptions, FauxResponseCallback, FauxResponseInput, FauxResponseOptions,
    FauxResponseStep, Registration, faux_assistant_message, lock, streaming, usage,
};
use crate::{
    AssistantMessage, AssistantMessageEventStream, Context, DiagnosticErrorInfo, Execution, Invoke,
    Local, Model, ProviderExecution, ProviderResponse, StopReason, Threaded,
};
use std::sync::{Arc, atomic::Ordering};

/// Original options and the response removed synchronously from the retained FIFO.
type Invocation<E> = (
    Context,
    Option<FauxResponseOptions<E>>,
    Option<FauxResponseStep>,
);
/// Factory result with its invocation's future representation.
type FactoryResult<E> =
    <E as Execution>::Future<'static, Result<AssistantMessage, DiagnosticErrorInfo>>;

/// Concrete future erasure beside the shared simulator worker.
pub(super) trait Producer: ProviderExecution {
    /// Box the same work without changing admission or factory execution.
    fn drive(
        registration: Arc<Registration>,
        stream: AssistantMessageEventStream,
        model: Arc<Model>,
        invocation: Invocation<Self>,
    ) -> Self::Future<'static, ()>;
}
/// Only the future's concrete auto-trait bounds differ.
macro_rules! producer {
    ($mode:ty) => {
        impl Producer for $mode {
            fn drive(
                registration: Arc<Registration>,
                stream: AssistantMessageEventStream,
                model: Arc<Model>,
                invocation: Invocation<Self>,
            ) -> Self::Future<'static, ()> {
                Box::pin(run::<Self>(registration, stream, model, invocation))
            }
        }
    };
}
producer!(Local);
producer!(Threaded);

/// Settle producer failures with the retained invocation identity.
async fn run<E: ProviderExecution>(
    registration: Arc<Registration>,
    stream: AssistantMessageEventStream,
    model: Arc<Model>,
    invocation: Invocation<E>,
) where
    dyn FauxResponseCallback: Invoke<E, FauxResponseInput<E>, Return = FactoryResult<E>>,
{
    if let Err(error) = registration
        .produce::<E>(stream.clone(), &model, invocation)
        .await
    {
        streaming::terminal(&stream, registration.error(&model, error.message));
    }
}

impl Registration {
    /// Consume a queued response synchronously, then submit its independent producer once.
    pub(super) fn invoke<E: Producer>(
        self: &Arc<Self>,
        model: Model,
        context: Context,
        options: Option<FauxResponseOptions<E>>,
        submit: E::Submission,
    ) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
        let step = lock(&self.pending).responses.pop_front();
        self.state.0.fetch_add(1, Ordering::AcqRel);
        let outer = AssistantMessageEventStream::new();
        let future = E::drive(
            Arc::clone(self),
            outer.clone(),
            Arc::new(model),
            (context, options, step),
        );
        E::submit(future, submit, streaming::spawn)?;
        Ok(outer)
    }
    /// Construct a zero-usage failure with invocation identity.
    fn error(&self, model: &Model, text: String) -> AssistantMessage {
        let mut message = faux_assistant_message(
            Vec::new(),
            FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::Error),
                error_message: Some(text),
                ..Default::default()
            },
        );
        message.api.clone_from(&self.api);
        message.provider.clone_from(&self.provider);
        message.model.clone_from(&model.id);
        message
    }
    /// Await response metadata, resolve the script and publish its events.
    async fn produce<E: ProviderExecution>(
        &self,
        outer: AssistantMessageEventStream,
        model: &Arc<Model>,
        invocation: Invocation<E>,
    ) -> Result<(), DiagnosticErrorInfo>
    where
        dyn FauxResponseCallback: Invoke<E, FauxResponseInput<E>, Return = FactoryResult<E>>,
    {
        let (context, options, step) = invocation;
        let common = options.as_ref().map(FauxResponseOptions::common);
        if let Some(hook) = common.and_then(|options| options.on_response.as_ref()) {
            hook(
                ProviderResponse {
                    status: 200.0,
                    headers: std::collections::BTreeMap::default(),
                },
                Arc::clone(model),
            )
            .await?;
        }
        let exhausted = step.is_none();
        let mut message = match step {
            Some(FauxResponseStep::Message(message)) => *message,
            Some(FauxResponseStep::Factory(factory)) => {
                Invoke::<E, _>::invoke(
                    factory.as_ref(),
                    FauxResponseInput {
                        context: context.clone(),
                        options: options.clone(),
                        state: self.state.clone(),
                        model: Model::clone(model),
                    },
                )
                .await?
            }
            None => self.error(model, "No more faux responses queued".into()),
        };
        message.api.clone_from(&self.api);
        message.provider.clone_from(&self.provider);
        message.model.clone_from(&model.id);
        usage::estimate(&mut message, &context, common, &self.pending)?;
        if exhausted {
            streaming::terminal(&outer, message);
        } else {
            streaming::deliver(
                &outer,
                message,
                &self.pacing,
                common.and_then(|value| value.signal.as_ref()),
            )
            .await?;
        }
        Ok(())
    }
}
