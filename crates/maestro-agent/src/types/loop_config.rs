//! Conversation inputs and model-boundary callbacks.
use super::{AgentMessage, CustomAgentMessages, SharedAgentTool};
use maestro_models::{JsonObject, Model, ProviderObjects, SimpleStreamOptions};
use std::sync::{Arc, RwLock};

/// Instructions and retained conversation inputs for one operation.
pub struct AgentContextRecord<
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Model instructions.
    pub system_prompt: String,
    /// Mutable ordered history.
    pub messages: Arc<RwLock<Vec<AgentMessage<C>>>>,
    /// Optional mutable executable-tool inventory.
    pub tools: Option<E::Shared<RwLock<Vec<SharedAgentTool<E>>>>>,
}
impl<C: CustomAgentMessages, E: super::AgentExecution> Clone for AgentContextRecord<C, E> {
    fn clone(&self) -> Self {
        Self {
            system_prompt: self.system_prompt.clone(),
            messages: Arc::clone(&self.messages),
            tools: self.tools.clone(),
        }
    }
}
/// Model selection and callbacks for each request.
pub struct AgentLoopConfigRecord<
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Optional validated-argument hook.
    pub before_tool_call: Option<super::BeforeToolCall<C, E>>,
    /// Optional execution-outcome hook.
    pub after_tool_call: Option<super::AfterToolCall<C, E>>,
    /// Optional completed-turn stop decision.
    pub should_stop_after_turn: Option<super::ShouldStopAfterTurn<C, E>>,
    /// Optional replacement of the next request's context, model and thinking level.
    pub prepare_next_turn: Option<super::PrepareNextTurn<C, E>>,
    /// Optional steering poll at entry and after a completed turn.
    pub get_steering_messages: Option<super::GetSteeringMessages<C, E>>,
    /// Optional follow-up poll when ordinary continuation is exhausted.
    pub get_follow_up_messages: Option<super::GetFollowUpMessages<C, E>>,
    /// Batch preference; absence selects parallel unless a tool requires sequential.
    pub tool_execution: Option<super::ToolExecutionMode>,
    /// Captured model handle: provider is read before key resolution, descriptor afterwards.
    pub model: Arc<RwLock<Model>>,
    /// Retained budgets snapshotted after credential resolution; absence keeps owned settings.
    pub thinking_budgets: Option<Arc<RwLock<maestro_models::ThinkingBudgets>>>,
    /// Simple invocation settings.
    pub options: SimpleStreamOptions<E>,
    /// Open settings passed to a replacement stream.
    pub extra: JsonObject,
    /// Typed objects passed to a replacement stream.
    pub objects: ProviderObjects,
    /// Convert conversation entries at the request boundary.
    pub convert_to_llm: ConvertToLlm<C, E>,
    /// Optional transformation before conversion.
    pub transform_context: TransformContext<C, E>,
    /// Optional per-request credential resolver.
    pub get_api_key: GetApiKey<E>,
}
/// Convert conversation entries at the model boundary.
pub type ConvertToLlm<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Convert<C>>;
/// Transform retained conversation entries.
pub type TransformContext<C, E = super::Threaded> =
    Option<<E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Transform<C>>>;
/// Resolve credentials per request.
pub type GetApiKey<E = super::Threaded> =
    Option<<E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Key>>;
/// Construct a replacement model stream.
pub type StreamFn<E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Stream>;

/// Thread-safe default representation of the `AgentContext` record.
pub type AgentContext<C = crate::NoCustomMessages> = AgentContextRecord<C, super::Threaded>;
/// Local representation of the `AgentContext` record.
pub type LocalAgentContext<C = crate::NoCustomMessages> = AgentContextRecord<C, super::Local>;

/// Thread-safe default representation of the `AgentLoopConfig` record.
pub type AgentLoopConfig<C = crate::NoCustomMessages> = AgentLoopConfigRecord<C, super::Threaded>;
/// Local representation of the `AgentLoopConfig` record.
pub type LocalAgentLoopConfig<C = crate::NoCustomMessages> = AgentLoopConfigRecord<C, super::Local>;
