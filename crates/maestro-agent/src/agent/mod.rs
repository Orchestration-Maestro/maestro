//! Conversation state ownership and queued input.
use crate::types::{AgentExecution, Threaded};
use crate::types::{AgentMessage, CustomAgentMessages, QueueMode, StreamFn};
use crate::{
    AfterToolCall, BeforeToolCall, ConvertToLlm, GetApiKey, PrepareNextTurn, ToolExecutionMode,
    TransformContext,
};
use maestro_listeners::Listeners;
use maestro_models::{OnPayload, OnResponse, ThinkingBudgets, Transport};
use std::sync::Mutex;
/// Prompt normalization and owned execution.
mod execution;
/// Runtime observations and settlement.
mod lifecycle;
/// Live ordered subscriptions.
mod listeners;
pub(crate) use execution::OwnedRun;
pub use execution::{AgentSpawner, PromptInput};
pub use listeners::{AgentListener, Subscription};
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};
/// Initialization and live state slots.
mod state;
pub use state::{AgentInitialState, AgentState};
/// Construction options for conversation state and queues.
pub struct AgentOptions<
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: AgentExecution = Threaded,
> {
    /// Context conversion captured at admission.
    pub convert_to_llm: Option<ConvertToLlm<C, E>>,
    /// Context transformation captured at admission.
    pub transform_context: TransformContext<C, E>,
    /// Model payload hook captured at admission.
    pub on_payload: Option<OnPayload<E>>,
    /// Model response hook captured at admission.
    pub on_response: Option<OnResponse<E>>,
    /// Validated tool hook captured at admission.
    pub before_tool_call: Option<BeforeToolCall<C, E>>,
    /// Executed tool hook captured at admission.
    pub after_tool_call: Option<AfterToolCall<C, E>>,
    /// Next-turn preparation captured at admission.
    pub prepare_next_turn: Option<PrepareNextTurn<C, E>>,
    /// Provider session identifier captured at admission.
    pub session_id: Option<String>,
    /// Transport preference captured at admission.
    pub transport: Option<Transport>,
    /// Provider retry cap captured at admission.
    pub max_retry_delay_ms: Option<f64>,
    /// Tool batch preference captured at admission.
    pub tool_execution: Option<ToolExecutionMode>,
    /// Per-request credential resolution captured at admission.
    pub get_api_key: GetApiKey<E>,
    /// Retained per-level budgets captured at admission.
    pub thinking_budgets: Option<Arc<RwLock<ThinkingBudgets>>>,
    /// Caller-owned initial state.
    pub initial_state: AgentInitialState<C, E>,
    /// Optional steering policy.
    pub steering_mode: Option<QueueMode>,
    /// Optional follow-up policy.
    pub follow_up_mode: Option<QueueMode>,
    /// Replacement stream captured at run admission.
    pub stream_fn: Option<StreamFn<E>>,
    /// Caller execution facility that retains submitted work independently.
    pub spawn: Option<AgentSpawner<E>>,
}
impl<C: CustomAgentMessages, E: AgentExecution> Default for AgentOptions<C, E> {
    fn default() -> Self {
        Self {
            convert_to_llm: None,
            transform_context: None,
            on_payload: None,
            on_response: None,
            before_tool_call: None,
            after_tool_call: None,
            prepare_next_turn: None,
            session_id: None,
            transport: None,
            max_retry_delay_ms: None,
            tool_execution: None,
            get_api_key: None,
            thinking_budgets: None,
            initial_state: AgentInitialState::default(),
            steering_mode: None,
            follow_up_mode: None,
            stream_fn: None,
            spawn: None,
        }
    }
}
/// Owner of live conversation state and independent input queues.
pub struct Agent<C: CustomAgentMessages = crate::NoCustomMessages, E: AgentExecution = Threaded> {
    /// Context conversion for subsequently admitted runs.
    pub convert_to_llm: RwLock<ConvertToLlm<C, E>>,
    /// Context transformation for subsequently admitted runs.
    pub transform_context: RwLock<TransformContext<C, E>>,
    /// Model payload hook for subsequently admitted runs.
    pub on_payload: RwLock<Option<OnPayload<E>>>,
    /// Model response hook for subsequently admitted runs.
    pub on_response: RwLock<Option<OnResponse<E>>>,
    /// Validated tool hook for subsequently admitted runs.
    pub before_tool_call: RwLock<Option<BeforeToolCall<C, E>>>,
    /// Executed tool hook for subsequently admitted runs.
    pub after_tool_call: RwLock<Option<AfterToolCall<C, E>>>,
    /// Next-turn preparation for subsequently admitted runs.
    pub prepare_next_turn: RwLock<Option<PrepareNextTurn<C, E>>>,
    /// Provider session identifier for subsequently admitted runs.
    pub session_id: RwLock<Option<String>>,
    /// Transport preference for subsequently admitted runs.
    pub transport: RwLock<Transport>,
    /// Provider retry cap for subsequently admitted runs.
    pub max_retry_delay_ms: RwLock<Option<f64>>,
    /// Tool batch preference for subsequently admitted runs.
    pub tool_execution: RwLock<ToolExecutionMode>,
    /// Credential resolver for subsequently admitted runs.
    pub get_api_key: RwLock<GetApiKey<E>>,
    /// Budget handle for subsequently admitted runs.
    pub thinking_budgets: RwLock<Option<Arc<RwLock<ThinkingBudgets>>>>,
    /// Shared live state.
    state: E::Shared<RwLock<AgentState<C, E>>>,
    /// Reserved run and independent completion observers.
    active: Mutex<Option<execution::ActiveRun>>,
    /// Live callback registrations.
    listeners: Mutex<Listeners<listeners::AgentListener<C, E>>>,
    /// Replacement stream captured at run admission.
    pub stream_fn: RwLock<Option<StreamFn<E>>>,
    /// Caller execution facility that retains submitted work independently.
    pub spawn: RwLock<Option<AgentSpawner<E>>>,

    /// Pending follow-up entries.
    follow_up: RwLock<VecDeque<AgentMessage<C>>>,
    /// Pending steering entries.
    steering: RwLock<VecDeque<AgentMessage<C>>>,
    /// Steering queue policy.
    steering_mode: RwLock<QueueMode>,
    /// Follow-up queue policy.
    follow_up_mode: RwLock<QueueMode>,
}
impl<C: CustomAgentMessages, E: AgentExecution> Default for Agent<C, E> {
    fn default() -> Self {
        Self::from_options(AgentOptions::default())
    }
}
impl<C: CustomAgentMessages, E: AgentExecution> Agent<C, E> {
    /// Construct state without invoking tools or providers.
    #[must_use]
    fn from_options(options: AgentOptions<C, E>) -> Self {
        Self {
            convert_to_llm: RwLock::new(
                options.convert_to_llm.unwrap_or_else(E::default_converter),
            ),
            transform_context: RwLock::new(options.transform_context),
            on_payload: RwLock::new(options.on_payload),
            on_response: RwLock::new(options.on_response),
            before_tool_call: RwLock::new(options.before_tool_call),
            after_tool_call: RwLock::new(options.after_tool_call),
            prepare_next_turn: RwLock::new(options.prepare_next_turn),
            session_id: RwLock::new(options.session_id),
            transport: RwLock::new(options.transport.unwrap_or(Transport::Auto)),
            max_retry_delay_ms: RwLock::new(options.max_retry_delay_ms),
            tool_execution: RwLock::new(
                options
                    .tool_execution
                    .unwrap_or(ToolExecutionMode::Parallel),
            ),
            get_api_key: RwLock::new(options.get_api_key),
            thinking_budgets: RwLock::new(options.thinking_budgets),
            active: Mutex::new(None),
            listeners: Mutex::new(Listeners::default()),
            stream_fn: RwLock::new(options.stream_fn),
            spawn: RwLock::new(options.spawn),
            follow_up: RwLock::new(VecDeque::new()),
            steering: RwLock::new(VecDeque::new()),
            state: E::share(RwLock::new(AgentState::new(options.initial_state))),
            steering_mode: RwLock::new(options.steering_mode.unwrap_or_default()),
            follow_up_mode: RwLock::new(options.follow_up_mode.unwrap_or_default()),
        }
    }
    /// The live state handle.
    pub fn state(&self) -> &E::Shared<RwLock<AgentState<C, E>>> {
        &self.state
    }
    /// The current steering policy.
    pub fn steering_mode(&self) -> QueueMode {
        *read(&self.steering_mode)
    }
    /// Replace the steering policy.
    pub fn set_steering_mode(&self, mode: QueueMode) {
        *write(&self.steering_mode) = mode;
    }
    /// The current follow-up policy.
    pub fn follow_up_mode(&self) -> QueueMode {
        *read(&self.follow_up_mode)
    }
    /// Replace the follow-up policy.
    pub fn set_follow_up_mode(&self, mode: QueueMode) {
        *write(&self.follow_up_mode) = mode;
    }
    /// Store steering input without appending it to history.
    pub fn steer(&self, message: AgentMessage<C>) {
        write(&self.steering).push_back(message);
    }
    /// Store follow-up input without appending it to history.
    pub fn follow_up(&self, message: AgentMessage<C>) {
        write(&self.follow_up).push_back(message);
    }
    /// Move steering entries according to the policy at this poll.
    pub(crate) fn drain_steering(&self) -> Vec<AgentMessage<C>> {
        Self::drain(&self.steering, self.steering_mode())
    }
    /// Move follow-up entries according to the policy at this poll.
    pub(crate) fn drain_follow_up(&self) -> Vec<AgentMessage<C>> {
        Self::drain(&self.follow_up, self.follow_up_mode())
    }
    /// Move entries without changing their allocation identity or order.
    fn drain(queue: &RwLock<VecDeque<AgentMessage<C>>>, mode: QueueMode) -> Vec<AgentMessage<C>> {
        let mut queue = write(queue);
        match mode {
            QueueMode::All => queue.drain(..).collect(),
            QueueMode::OneAtATime => queue.pop_front().into_iter().collect(),
        }
    }
    /// Clear steering input, releasing retired entries after unlocking.
    pub fn clear_steering_queue(&self) {
        let retired = std::mem::take(&mut *write(&self.steering));
        drop(retired);
    }
    /// Clear follow-up input, releasing retired entries after unlocking.
    pub fn clear_follow_up_queue(&self) {
        let retired = std::mem::take(&mut *write(&self.follow_up));
        drop(retired);
    }
    /// Clear both input queues.
    pub fn clear_all_queues(&self) {
        self.clear_steering_queue();
        self.clear_follow_up_queue();
    }
    /// Clear history and runtime observations while preserving configuration.
    pub fn reset(&self) {
        let retired = {
            let mut state = write(&self.state);
            state.is_streaming = false;
            (
                std::mem::replace(&mut state.messages, Arc::new(RwLock::new(Vec::new()))),
                state.streaming_message.take(),
                std::mem::replace(&mut state.pending_tool_calls, Arc::from([])),
                state.error_message.take(),
            )
        };
        drop(retired);
        self.clear_all_queues();
    }
    /// Whether either input queue contains entries.
    pub fn has_queued_messages(&self) -> bool {
        !read(&self.steering).is_empty() || !read(&self.follow_up).is_empty()
    }
}

#[cfg(test)]
mod tests;

/// Read a retained lock, recovering its value after poisoning.
pub(super) fn read<T>(v: &RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    v.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Write a retained lock, recovering its value after poisoning.
pub(super) fn write<T>(v: &RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    v.write().unwrap_or_else(std::sync::PoisonError::into_inner)
}
/// Acquire internal ownership, recovering after poisoning.
pub(super) fn lock<T>(v: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    v.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
#[cfg(not(target_arch = "wasm32"))]
/// Target-specific bounds for owned execution.
pub trait Native: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> Native for T {}
#[cfg(target_arch = "wasm32")]
/// Target-specific bounds for owned execution.
pub trait Native {}
#[cfg(target_arch = "wasm32")]
impl<T> Native for T {}

impl<C: CustomAgentMessages> Agent<C, Threaded> {
    /// Construct the existing SDK owner.
    #[must_use]
    pub fn new(options: AgentOptions<C, Threaded>) -> Self {
        Self::from_options(options)
    }
}
impl<C: CustomAgentMessages> Agent<C, crate::Local> {
    /// Construct a local owner with caller-owned submission.
    #[must_use]
    pub fn new_local(options: AgentOptions<C, crate::Local>) -> Self {
        Self::from_options(options)
    }
}
