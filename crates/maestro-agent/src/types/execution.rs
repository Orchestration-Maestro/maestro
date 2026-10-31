//! Compile-time callback affinity for owned agent execution.
use super::{
    AfterToolCallContext, AfterToolCallResult, AgentEvent, AgentLoopConfigRecord,
    AgentLoopTurnUpdate, AgentMessage, AgentToolResult, AgentToolUpdateCallback,
    BeforeToolCallContext, BeforeToolCallResult, ConvertToLlm, CustomAgentMessages, ExecuteTool,
    PrepareNextTurnContext, ShouldStopAfterTurnContext,
};
use maestro_models::{
    AssistantMessageEventStream, Cancellation, Context, DiagnosticErrorInfo, Message, Model,
    ProviderStreamOptions, SchemaCodeSource, SimpleStreamOptions,
};
use maestro_models::{Local, Threaded};
use std::{future::Future, rc::Rc, sync::Arc};
/// Fallible callback work in the selected future family.
type CallbackFuture<'a, E, T> =
    <E as maestro_models::Execution>::Future<'a, Result<T, DiagnosticErrorInfo>>;
/// Agent callback signatures for the lower-owned execution representation.
pub trait AgentExecution: maestro_models::ProviderExecution {
    /// Executable annotations follow storage affinity; threaded defaults keep only data.
    type SchemaCode: maestro_models::SchemaCodeSource + Clone + Default;
    /// Project retained metadata into local invocation without calling author code.
    fn local_definition(
        definition: &maestro_models::Tool<Self::SchemaCode>,
    ) -> maestro_models::Tool<maestro_models::SchemaCode> {
        maestro_models::Tool {
            name: definition.name.clone(),
            description: definition.description.clone(),
            parameters: definition.parameters.data().with_code(
                definition
                    .parameters
                    .code()
                    .schema_code()
                    .cloned()
                    .unwrap_or_default(),
            ),
        }
    }
    /// A retained observation future with no borrowed operation data.
    type ObservationFuture: Future<Output = Result<(), DiagnosticErrorInfo>> + Unpin + 'static;
    /// A live model-facing tool description.
    type Description: Fn() -> Result<String, DiagnosticErrorInfo> + ?Sized;
    /// A synchronous tool progress observer.
    type Update<D>: Fn(AgentToolResult<D>) + ?Sized;
    /// The argument preparation callable.
    type Prepare: Fn(&mut serde_json::Value) -> Result<serde_json::Value, DiagnosticErrorInfo>
        + ?Sized;
    /// The typed tool execution callable.
    type Execute<P, D>: Fn(
            String,
            P,
            Option<Cancellation>,
            Option<AgentToolUpdateCallback<D, Self>>,
        ) -> CallbackFuture<'static, Self, AgentToolResult<D>>
        + ?Sized;
    /// The borrowed pre-execution hook callable.
    type BeforeToolCall<C: CustomAgentMessages>: for<'a> Fn(
            BeforeToolCallContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<BeforeToolCallResult>>
        + ?Sized;
    /// The borrowed result-editing hook callable.
    type AfterToolCall<C: CustomAgentMessages>: for<'a> Fn(
            AfterToolCallContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<AfterToolCallResult>>
        + ?Sized;
    /// The borrowed turn-stopping callable.
    type Stop<C: CustomAgentMessages>: for<'a> Fn(ShouldStopAfterTurnContext<'a, C, Self>) -> CallbackFuture<'a, Self, bool>
        + ?Sized;
    /// The borrowed next-turn preparation callable.
    type PrepareNextTurn<C: CustomAgentMessages>: for<'a> Fn(
            PrepareNextTurnContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<AgentLoopTurnUpdate<C, Self>>>
        + ?Sized;
    /// The queued-input polling callable.
    type Poll<C: CustomAgentMessages>: Fn() -> CallbackFuture<'static, Self, Vec<AgentMessage<C>>>
        + ?Sized;
    /// The message projection callable.
    type Convert<C: CustomAgentMessages>: Fn(
            Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>,
        ) -> CallbackFuture<'static, Self, Vec<Message>>
        + ?Sized;
    /// The message-history transformation callable.
    type Transform<C: CustomAgentMessages>: Fn(
            Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>,
            Option<Cancellation>,
        ) -> CallbackFuture<'static, Self, Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>>
        + ?Sized;
    /// The credential resolution callable.
    type Key: Fn(String) -> CallbackFuture<'static, Self, Option<String>> + ?Sized;
    /// The replacement model-stream callable.
    type Stream: Fn(
            Model,
            Context,
            SimpleStreamOptions<Self>,
            ProviderStreamOptions<Self>,
        ) -> CallbackFuture<'static, Self, AssistantMessageEventStream>
        + ?Sized;
    /// The owned event observation callable.
    type Sink<C: CustomAgentMessages>: Fn(AgentEvent<C>) -> Self::ObservationFuture + ?Sized;
    /// The shared live-listener callable.
    type Listener<C: CustomAgentMessages>: Fn(Arc<std::sync::RwLock<AgentEvent<C>>>, Cancellation) -> Self::ObservationFuture
        + ?Sized;
    /// The caller-owned execution admission callable.
    type Spawner: Fn(Self::Future<'static, ()>) + ?Sized;
    /// Adapt the existing task facility for provider admission.
    fn provider_submission(spawn: crate::AgentSpawner<Self>) -> Self::Submission;
    /// Dispatch with the selected family's existing default or supplied admission.
    ///
    /// # Errors
    /// Returns the model adapter's lookup or setup error.
    fn provider_stream(
        model: Model,
        context: Context,
        options: SimpleStreamOptions<Self>,
        submission: Self::Submission,
    ) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    /// Construct the default message projection.
    fn default_converter<C: CustomAgentMessages>() -> ConvertToLlm<C, Self>;
    /// Erase typed arguments and details at the existing tool boundary.
    fn erase_execute<P: serde::de::DeserializeOwned + 'static, D: serde::Serialize + 'static>(
        execute: ExecuteTool<P, D, Self>,
    ) -> ExecuteTool<serde_json::Value, serde_json::Value, Self>;
}
/// One declaration template; only pointer and auto-trait bounds differ.
macro_rules! family {
 ($mode:ty,$ptr:ident, [$($bound:tt)*], $code:ty, $submission:expr, $dispatch:expr) => {

impl AgentExecution for $mode {
    type SchemaCode = $code;
    type ObservationFuture = <Self as maestro_models::Execution>::Future<'static,Result<(),DiagnosticErrorInfo>>;
    type Description = dyn Fn() -> Result<String, DiagnosticErrorInfo> $($bound)*;
    type Update<D> = dyn Fn(AgentToolResult<D>) $($bound)*;
    type Prepare = dyn Fn(&mut serde_json::Value) -> Result<serde_json::Value, DiagnosticErrorInfo>
        $($bound)*;
    type Execute<P, D> = dyn Fn(
            String,
            P,
            Option<Cancellation>,
            Option<AgentToolUpdateCallback<D, Self>>,
        ) -> CallbackFuture<'static, Self, AgentToolResult<D>>
        $($bound)*;
    type BeforeToolCall<C: CustomAgentMessages> = dyn for<'a> Fn(
            BeforeToolCallContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<BeforeToolCallResult>>
        $($bound)*;
    type AfterToolCall<C: CustomAgentMessages> = dyn for<'a> Fn(
            AfterToolCallContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<AfterToolCallResult>>
        $($bound)*;
    type Stop<C: CustomAgentMessages> = dyn for<'a> Fn(ShouldStopAfterTurnContext<'a, C, Self>) -> CallbackFuture<'a, Self, bool>
        $($bound)*;
    type PrepareNextTurn<C: CustomAgentMessages> = dyn for<'a> Fn(
            PrepareNextTurnContext<'a, C, Self>,
            Option<Cancellation>,
        ) -> CallbackFuture<'a, Self, Option<AgentLoopTurnUpdate<C, Self>>>
        $($bound)*;
    type Poll<C: CustomAgentMessages> =
        dyn Fn() -> CallbackFuture<'static, Self, Vec<AgentMessage<C>>> $($bound)*;
    type Convert<C: CustomAgentMessages> = dyn Fn(
            Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>,
        ) -> CallbackFuture<'static, Self, Vec<Message>>
        $($bound)*;
    type Transform<C: CustomAgentMessages> = dyn Fn(
            Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>,
            Option<Cancellation>,
        ) -> CallbackFuture<'static, Self, Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>>
        $($bound)*;
    type Key = dyn Fn(String) -> CallbackFuture<'static, Self, Option<String>> $($bound)*;
    type Stream = dyn Fn(
            Model,
            Context,
            SimpleStreamOptions<Self>,
            ProviderStreamOptions<Self>,
        ) -> CallbackFuture<'static, Self, AssistantMessageEventStream>
        $($bound)*;
    type Sink<C: CustomAgentMessages> =
        dyn Fn(AgentEvent<C>) -> Self::ObservationFuture $($bound)*;
    type Listener<C: CustomAgentMessages> = dyn Fn(Arc<std::sync::RwLock<AgentEvent<C>>>, Cancellation) -> Self::ObservationFuture
        $($bound)*;
    type Spawner = dyn Fn(Self::Future<'static, ()>) $($bound)*;
    fn provider_submission(spawn: crate::AgentSpawner<Self>) -> Self::Submission {
        ($submission)(spawn)
    }
    fn provider_stream(model: Model, context: Context, options: SimpleStreamOptions<Self>, submission: Self::Submission) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
        ($dispatch)(model, context, options, submission)
    }
    fn default_converter<C: CustomAgentMessages>() -> ConvertToLlm<C, Self> {
        $ptr::new(|history| {
            let messages = crate::agent::read(&history)
                .iter()
                .filter_map(AgentMessage::to_message)
                .collect();
            Box::pin(std::future::ready(Ok(messages)))
        })
    }
    fn erase_execute<P: serde::de::DeserializeOwned + 'static, D: serde::Serialize + 'static>(
        execute: ExecuteTool<P, D, Self>,
    ) -> ExecuteTool<serde_json::Value, serde_json::Value, Self> {
        $ptr::new(move |id, args, signal, update| {
            let parameters = match serde_json::from_value(args) {
                Ok(p) => p,
                Err(e) => return Box::pin(std::future::ready(Err(super::tools::json_error(&e)))),
            };
            let failure = Arc::new(std::sync::RwLock::new(None));
            let progress_failure = failure.clone();
            let progress = update.map(|update| {
                $ptr::new(move |result| match super::tools::encode_result(result) {
                    Ok(result) => update(result),
                    Err(error) => *crate::agent::write(&progress_failure) = Some(error),
                }) as AgentToolUpdateCallback<D, Self>
            });
            let result = execute(id, parameters, signal, progress);
            Box::pin(async move {
                let result = result.await?;
                if let Some(error) = crate::agent::write(&failure).take() {
                    return Err(error);
                }
                super::tools::encode_result(result)
            })
        })
    }
}
 };
}
family!(
    Local,
    Rc,
    [],
    maestro_models::SchemaCode,
    |spawn: crate::AgentSpawner<Local>| Rc::new(move |task| {
        spawn(task);
        true
    }),
    |model, context, options, submit| maestro_models::stream_simple_for::<Local>(
        model,
        context,
        Some(options),
        submit
    )
);
#[cfg(not(target_arch = "wasm32"))]
family!(Threaded,Arc,[+ Send + Sync], (), |_spawn| None, threaded_provider_stream);
#[cfg(target_arch = "wasm32")]
family!(
    Threaded,
    Rc,
    [],
    (),
    |_spawn| None,
    threaded_provider_stream
);
/// Preserve the ordinary provider default unless an override was supplied.
fn threaded_provider_stream(
    model: Model,
    context: Context,
    options: SimpleStreamOptions,
    submission: <Threaded as maestro_models::ProviderExecution>::Submission,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    match submission {
        Some(spawn) => {
            maestro_models::stream_simple_for::<Threaded>(model, context, Some(options), spawn)
        }
        None => maestro_models::stream_simple(model, context, Some(options)),
    }
}
/// Concrete callable-erasure and task-submission glue.
pub trait ExecutionFor<C: CustomAgentMessages + 'static>: AgentExecution {
    /// Concurrently polled admitted observations.
    type ProgressStream: futures_util::Stream<Item = Result<(), DiagnosticErrorInfo>>
        + Unpin
        + 'static;
    /// Erase the existing concurrent observation stream.
    fn pending_progress(
        updates: maestro_models::EventStream<Self::ObservationFuture, ()>,
    ) -> Self::ProgressStream;
    /// Erase direct reduction and live-listener invocation.
    fn agent_sink(
        agent: Self::Shared<crate::Agent<C, Self>>,
        signal: Cancellation,
    ) -> crate::AgentEventSink<C, Self>;
    /// Erase delivery to the existing event stream.
    fn stream_sink(
        stream: maestro_models::EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>,
    ) -> crate::AgentEventSink<C, Self>;
    /// Erase the common owned-operation future.
    fn owned(work: crate::agent::OwnedRun<C, Self>) -> Self::Future<'static, ()>;
    /// Erase the common submitted-stream future.
    fn submitted(work: crate::agent_loop::Submitted<C, Self>) -> Self::Future<'static, ()>;
    /// Erase direct synchronous progress admission.
    fn progress(
        work: crate::agent_loop::progress::Progress<C, Self>,
    ) -> AgentToolUpdateCallback<serde_json::Value, Self>;
    /// Bind the existing steering and follow-up queues.
    fn install_queues(
        agent: Self::Shared<crate::Agent<C, Self>>,
        config: &mut AgentLoopConfigRecord<C, Self>,
        skip: bool,
    );
}
/// Concrete erasure delegates to shared work owners.
macro_rules! run_glue {
 ($mode:ty,$ptr:ident,[$($extra:tt)*],$boxed:ident,$stream:ty)=>{

impl<C: CustomAgentMessages + 'static $($extra)*> ExecutionFor<C> for $mode {
    type ProgressStream = $stream;
    fn pending_progress(
        updates: maestro_models::EventStream<Self::ObservationFuture, ()>,
    ) -> Self::ProgressStream {
        use futures_util::StreamExt;
        let pending = futures_util::stream::unfold(updates, |updates| async move {
            updates.next().await.map(|future| (future, updates))
        })
        .$boxed();
        pending
            .map(futures_util::stream::once)
            .flatten_unordered(None)
            .$boxed()
    }
    fn agent_sink(
        agent: Self::Shared<crate::Agent<C, Self>>,
        signal: Cancellation,
    ) -> crate::AgentEventSink<C, Self> {
        $ptr::new(move |event| {
            Box::pin(crate::Agent::<C, Self>::emit(
                agent.clone(),
                event,
                signal.clone(),
            ))
        })
    }
    fn stream_sink(
        stream: maestro_models::EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>,
    ) -> crate::AgentEventSink<C, Self> {
        $ptr::new(move |event| {
            stream.push(event);
            Box::pin(std::future::ready(Ok(())))
        })
    }
    fn owned(work: crate::agent::OwnedRun<C, Self>) -> Self::Future<'static, ()> {
        Box::pin(work.drive())
    }
    fn submitted(work: crate::agent_loop::Submitted<C, Self>) -> Self::Future<'static, ()> {
        Box::pin(work.drive())
    }
    fn progress(
        work: crate::agent_loop::progress::Progress<C, Self>,
    ) -> AgentToolUpdateCallback<serde_json::Value, Self> {
        $ptr::new(move |partial| work.update(partial))
    }
    fn install_queues(
        agent: Self::Shared<crate::Agent<C, Self>>,
        config: &mut AgentLoopConfigRecord<C, Self>,
        skip: bool,
    ) {
        let steering = agent.clone();
        let skip = std::sync::atomic::AtomicBool::new(skip);
        config.get_steering_messages = Some($ptr::new(move || {
            let messages = if skip.swap(false, std::sync::atomic::Ordering::Relaxed) {
                Vec::new()
            } else {
                steering.drain_steering()
            };
            Box::pin(std::future::ready(Ok(messages)))
        }));
        config.get_follow_up_messages = Some($ptr::new(move || {
            Box::pin(std::future::ready(Ok(agent.drain_follow_up())))
        }));
    }
}
 };
}
run_glue!(
    Local,
    Rc,
    [],
    boxed_local,
    futures_util::stream::LocalBoxStream<'static, Result<(), DiagnosticErrorInfo>>
);
#[cfg(not(target_arch = "wasm32"))]
run_glue!(Threaded,Arc,[+crate::agent::Native],boxed,futures_util::stream::BoxStream<'static,Result<(),DiagnosticErrorInfo>>);
#[cfg(target_arch = "wasm32")]
run_glue!(
    Threaded,
    Rc,
    [],
    boxed_local,
    futures_util::stream::LocalBoxStream<'static, Result<(), DiagnosticErrorInfo>>
);
