#![doc = include_str!("../../../../../docs/models/faux.md")]
/// Owned content, message and model constructors.
mod builders;
use crate::{
    ApiProvider, AssistantContent, AssistantMessage, AssistantMessageEventStream, Context,
    DiagnosticErrorInfo, Execution, Invoke, Local, Model, ModelCost, ModelInput, ProviderExecution,
    ProviderResponse, RegistryCapture, StopReason, StreamOptionsInput, TextContent,
    ThinkingContent, Threaded, ToolCall, register_api_provider, unregister_api_providers,
};
pub use builders::{faux_assistant_message, faux_text, faux_thinking, faux_tool_call};

/// One scripted assistant content block.
pub type FauxContentBlock = AssistantContent;
/// Optional tool identifier.
#[derive(Clone, Default)]
pub struct FauxToolCallOptions {
    /// Explicit identifier, including an empty identifier.
    pub id: Option<String>,
}
/// Optional assistant metadata.
#[derive(Clone, Default)]
pub struct FauxAssistantMessageOptions {
    /// Terminal outcome.
    pub stop_reason: Option<StopReason>,
    /// Authored failure text.
    pub error_message: Option<String>,
    /// Response identity.
    pub response_id: Option<String>,
    /// Milliseconds since the epoch.
    pub timestamp: Option<f64>,
}
/// Accepted assistant input forms.
#[derive(Clone)]
pub enum FauxAssistantContent {
    /// Plain text.
    Text(String),
    /// One block.
    Block(FauxContentBlock),
    /// Ordered blocks.
    Blocks(Vec<FauxContentBlock>),
}
impl From<String> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<&str> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: &str) -> Self {
        Self::Text(value.into())
    }
}
impl From<FauxContentBlock> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: FauxContentBlock) -> Self {
        Self::Block(value)
    }
}
impl From<Vec<FauxContentBlock>> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: Vec<FauxContentBlock>) -> Self {
        Self::Blocks(value)
    }
}
impl From<TextContent> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: TextContent) -> Self {
        Self::Block(AssistantContent::Text(value))
    }
}
impl From<ThinkingContent> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: ThinkingContent) -> Self {
        Self::Block(AssistantContent::Thinking(value))
    }
}
impl From<ToolCall> for FauxAssistantContent {
    /// Convert a supported input into its assistant content variant.
    fn from(value: ToolCall) -> Self {
        Self::Block(AssistantContent::ToolCall(value))
    }
}

/// Typed event delivery and target-local scheduling.
mod streaming;
/// Prompt projection and disjoint session-cache accounting.
mod usage;
use std::collections::{HashMap, VecDeque};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicUsize, Ordering},
};

/// Overrides for one registered model.
#[derive(Clone, Default)]
pub struct FauxModelDefinition {
    /// Model identifier.
    pub id: String,
    /// Display name, defaulting to the identifier.
    pub name: Option<String>,
    /// Whether the model supports thinking.
    pub reasoning: Option<bool>,
    /// Accepted input kinds.
    pub input: Option<Vec<ModelInput>>,
    /// Per-million token prices.
    pub cost: Option<ModelCost>,
    /// Context capacity.
    pub context_window: Option<f64>,
    /// Output capacity.
    pub max_tokens: Option<f64>,
}
/// Chunk size range in estimated tokens.
#[derive(Clone, Default)]
pub struct FauxTokenSize {
    /// Lower bound, defaulting to three.
    pub min: Option<f64>,
    /// Upper bound, defaulting to five.
    pub max: Option<f64>,
}
/// Registration overrides and delivery pacing.
#[derive(Clone, Default)]
pub struct RegisterFauxProviderOptions {
    /// API identity, generated when absent.
    pub api: Option<String>,
    /// Provider identity, defaulting to faux.
    pub provider: Option<String>,
    /// Model definitions; absent or empty uses the default model.
    pub models: Option<Vec<FauxModelDefinition>>,
    /// Positive delivery rate; otherwise delivery is unpaced.
    pub tokens_per_second: Option<f64>,
    /// Chunk range overrides.
    pub token_size: Option<FauxTokenSize>,
}
/// Complete original invocation options received by a response factory.
pub type FauxResponseOptions<E = Threaded> = StreamOptionsInput<E>;
/// Original invocation values retained by an asynchronous response factory.
pub struct FauxResponseInput<E: ProviderExecution> {
    /// Conversation supplied to this request.
    pub context: Context,
    /// Absent or supplied original raw/simple options.
    pub options: Option<FauxResponseOptions<E>>,
    /// Live registration state.
    pub state: FauxProviderState,
    /// Selected model descriptor.
    pub model: Model,
}
/// Live registration call count shared with factories.
#[derive(Clone)]
pub struct FauxProviderState(Arc<AtomicUsize>);
impl FauxProviderState {
    /// Number of requests consumed, including exhausted requests.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.0.load(Ordering::Acquire)
    }
}
/// Retained asynchronous factory callable with either request family.
pub trait FauxResponseCallback:
    Invoke<
        Local,
        FauxResponseInput<Local>,
        Return = <Local as Execution>::Future<
            'static,
            Result<AssistantMessage, DiagnosticErrorInfo>,
        >,
    > + Invoke<
        Threaded,
        FauxResponseInput<Threaded>,
        Return = <Threaded as Execution>::Future<
            'static,
            Result<AssistantMessage, DiagnosticErrorInfo>,
        >,
    > + RegistryCapture
{
}
impl<T> FauxResponseCallback for T where
    T: Invoke<
            Local,
            FauxResponseInput<Local>,
            Return = <Local as Execution>::Future<
                'static,
                Result<AssistantMessage, DiagnosticErrorInfo>,
            >,
        > + Invoke<
            Threaded,
            FauxResponseInput<Threaded>,
            Return = <Threaded as Execution>::Future<
                'static,
                Result<AssistantMessage, DiagnosticErrorInfo>,
            >,
        > + RegistryCapture
{
}
/// One retained factory whose future follows the invocation family.
pub type FauxResponseFactory = Arc<dyn FauxResponseCallback>;
/// One queued response or asynchronous factory.
#[derive(Clone)]
pub enum FauxResponseStep {
    /// Owned fixed response.
    Message(Box<AssistantMessage>),
    /// Factory resolved after the response hook.
    Factory(FauxResponseFactory),
}
/// Registration-local FIFO and prompt cache.
struct Pending {
    /// Owned scripts awaiting invocation.
    responses: VecDeque<FauxResponseStep>,
    /// Last full prompt for each enabled session.
    cache: HashMap<String, String>,
}
/// Private producer state retained independently of observers.
struct Registration {
    /// Registered protocol identity.
    api: String,
    /// Registered provider identity.
    provider: String,
    /// Live request count.
    state: FauxProviderState,
    /// Queued scripts and session prompts.
    pending: Mutex<Pending>,
    /// Normalized chunk sizes and optional rate.
    pacing: streaming::Pacing,
}
/// Acquire registration state, recovering poisoned ownership.
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Registered models and their scripted response queue.
pub struct FauxProviderRegistration {
    /// Registered API identity.
    pub api: String,
    /// Registered model descriptors in declaration order.
    pub models: Vec<Model>,
    /// Live request count.
    pub state: FauxProviderState,
    /// Retained producer state.
    registration: crate::SharedHandle<Registration>,
    /// Registry ownership identity used for explicit cleanup.
    source_id: String,
}
impl FauxProviderRegistration {
    /// Select the first model for an absent or empty identifier.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<&Model> {
        match model_id.filter(|id| !id.is_empty()) {
            None => self.models.first(),
            Some(id) => self.models.iter().find(|model| model.id == id),
        }
    }
    /// Replace pending responses without clearing cache or call count.
    pub fn set_responses(&self, responses: Vec<FauxResponseStep>) {
        let retired = std::mem::replace(
            &mut lock(&self.registration.pending).responses,
            responses.into(),
        );
        drop(retired);
    }
    /// Append responses in FIFO order.
    pub fn append_responses(&self, responses: Vec<FauxResponseStep>) {
        lock(&self.registration.pending).responses.extend(responses);
    }
    /// Return the current pending response count.
    #[must_use]
    pub fn get_pending_response_count(&self) -> usize {
        lock(&self.registration.pending).responses.len()
    }
    /// Remove only registrations still owned by this handle.
    pub fn unregister(&self) {
        unregister_api_providers(&self.source_id);
    }
}
impl Registration {
    /// Consume a queued response synchronously and prepare its independent producer.
    fn invocation(
        self: &crate::SharedHandle<Self>,
        model: Model,
        context: Context,
        options: Option<FauxResponseOptions>,
    ) -> (AssistantMessageEventStream, crate::BoxFuture<()>) {
        let step = lock(&self.pending).responses.pop_front();
        self.state.0.fetch_add(1, Ordering::AcqRel);
        let outer = AssistantMessageEventStream::new();
        let producer = outer.clone();
        let registration = crate::SharedHandle::clone(self);
        let model = Arc::new(model);
        let future = Box::pin(async move {
            if let Err(error) = registration
                .produce(producer.clone(), &model, (context, options, step))
                .await
            {
                streaming::terminal(&producer, registration.error(&model, error.message));
            }
        });
        (outer, future)
    }
    /// Launch the native producer while preserving setup failure.
    #[cfg(not(target_arch = "wasm32"))]
    fn invoke(
        self: &crate::SharedHandle<Self>,
        model: Model,
        context: Context,
        options: Option<FauxResponseOptions>,
    ) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
        let (outer, future) = self.invocation(model, context, options);
        streaming::spawn(future)?;
        Ok(outer)
    }
    /// Launch the browser-local producer.
    #[cfg(target_arch = "wasm32")]
    fn invoke(
        self: &crate::SharedHandle<Self>,
        model: Model,
        context: Context,
        options: Option<FauxResponseOptions>,
    ) -> AssistantMessageEventStream {
        let (outer, future) = self.invocation(model, context, options);
        streaming::spawn(future);
        outer
    }
    /// Retain raw and simple callbacks over this registration.
    fn provider(self: &crate::SharedHandle<Self>) -> ApiProvider {
        let raw = crate::SharedHandle::clone(self);
        let simple = crate::SharedHandle::clone(self);
        ApiProvider {
            api: self.api.clone(),
            stream: crate::SharedHandle::new(move |model, context, options| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    raw.invoke(model, context, options.map(FauxResponseOptions::Raw))
                }
                #[cfg(target_arch = "wasm32")]
                {
                    Ok(raw.invoke(model, context, options.map(FauxResponseOptions::Raw)))
                }
            }),
            stream_simple: crate::SharedHandle::new(move |model, context, options| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    simple.invoke(model, context, options.map(FauxResponseOptions::Simple))
                }
                #[cfg(target_arch = "wasm32")]
                {
                    Ok(simple.invoke(model, context, options.map(FauxResponseOptions::Simple)))
                }
            }),
        }
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
    async fn produce(
        &self,
        outer: AssistantMessageEventStream,
        model: &Arc<Model>,
        invocation: (
            Context,
            Option<FauxResponseOptions>,
            Option<FauxResponseStep>,
        ),
    ) -> Result<(), DiagnosticErrorInfo> {
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
                Invoke::<Threaded, _>::invoke(
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
/// Register an opt-in provider with an initially empty response queue.
#[must_use]
pub fn register_faux_provider(options: RegisterFauxProviderOptions) -> FauxProviderRegistration {
    let api = options.api.unwrap_or_else(|| builders::random_id("faux"));
    let provider = options.provider.unwrap_or_else(|| "faux".into());
    let source_id = builders::random_id("faux-provider");
    let state = FauxProviderState(Arc::new(AtomicUsize::new(0)));
    let definitions = options
        .models
        .filter(|models| !models.is_empty())
        .unwrap_or_else(|| {
            vec![FauxModelDefinition {
                id: "faux-1".into(),
                name: Some("Faux Model".into()),
                ..Default::default()
            }]
        });
    let models = definitions
        .into_iter()
        .map(|definition| builders::model(definition, &api, &provider))
        .collect();
    let registration = crate::SharedHandle::new(Registration {
        api: api.clone(),
        provider,
        state: state.clone(),
        pending: Mutex::new(Pending {
            responses: VecDeque::new(),
            cache: HashMap::new(),
        }),
        pacing: streaming::Pacing::new(
            &options.token_size.unwrap_or_default(),
            options.tokens_per_second,
        ),
    });
    register_api_provider(registration.provider(), Some(source_id.clone()));
    FauxProviderRegistration {
        api,
        models,
        state,
        registration,
        source_id,
    }
}
