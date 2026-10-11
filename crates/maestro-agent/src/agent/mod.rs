//! Conversation state ownership and queued input.
use crate::types::{AgentMessage, CustomAgentMessages, QueueMode, StreamFn};
use std::sync::Mutex;
/// Prompt normalization and owned execution.
mod execution;
/// Runtime observations and settlement.
mod lifecycle;
/// Live ordered subscriptions.
mod listeners;
pub use execution::{AgentSpawner, PromptInput};
pub use listeners::{AgentListener, Subscription};
use std::{
    collections::VecDeque,
    convert::Infallible,
    sync::{Arc, RwLock},
};
/// Initialization and live state slots.
mod state;
pub use state::{AgentInitialState, AgentState};
/// Construction options for conversation state and queues.
pub struct AgentOptions<C: CustomAgentMessages = Infallible> {
    /// Caller-owned initial state.
    pub initial_state: AgentInitialState<C>,
    /// Optional steering policy.
    pub steering_mode: Option<QueueMode>,
    /// Optional follow-up policy.
    pub follow_up_mode: Option<QueueMode>,
    /// Replacement stream captured at prompt admission.
    pub stream_fn: Option<StreamFn>,
    /// Caller execution facility that retains submitted work independently.
    pub spawn: Option<AgentSpawner>,
}
impl<C: CustomAgentMessages> Default for AgentOptions<C> {
    fn default() -> Self {
        Self {
            initial_state: AgentInitialState::default(),
            steering_mode: None,
            follow_up_mode: None,
            stream_fn: None,
            spawn: None,
        }
    }
}
/// Owner of live conversation state and independent input queues.
pub struct Agent<C: CustomAgentMessages = Infallible> {
    /// Shared live state.
    state: Arc<RwLock<AgentState<C>>>,
    /// Reserved run and independent completion observers.
    active: Mutex<Option<execution::ActiveRun>>,
    /// Live callback registrations.
    listeners: Mutex<listeners::Listeners<C>>,
    /// Replacement stream captured at prompt admission.
    pub stream_fn: RwLock<Option<StreamFn>>,
    /// Caller execution facility that retains submitted work independently.
    pub spawn: RwLock<Option<AgentSpawner>>,

    /// Pending follow-up entries.
    follow_up: RwLock<VecDeque<AgentMessage<C>>>,
    /// Pending steering entries.
    steering: RwLock<VecDeque<AgentMessage<C>>>,
    /// Steering queue policy.
    steering_mode: RwLock<QueueMode>,
    /// Follow-up queue policy.
    follow_up_mode: RwLock<QueueMode>,
}
impl<C: CustomAgentMessages> Default for Agent<C> {
    fn default() -> Self {
        Self::new(AgentOptions::default())
    }
}
impl<C: CustomAgentMessages> Agent<C> {
    /// Construct state without invoking tools or providers.
    #[must_use]
    pub fn new(options: AgentOptions<C>) -> Self {
        Self {
            active: Mutex::new(None),
            listeners: Mutex::new(listeners::Listeners::default()),
            stream_fn: RwLock::new(options.stream_fn),
            spawn: RwLock::new(options.spawn),
            follow_up: RwLock::new(VecDeque::new()),
            steering: RwLock::new(VecDeque::new()),
            state: Arc::new(RwLock::new(AgentState::new(options.initial_state))),
            steering_mode: RwLock::new(options.steering_mode.unwrap_or_default()),
            follow_up_mode: RwLock::new(options.follow_up_mode.unwrap_or_default()),
        }
    }
    /// The live state handle.
    pub fn state(&self) -> &Arc<RwLock<AgentState<C>>> {
        &self.state
    }
    /// The current steering policy.
    pub fn steering_mode(&self) -> QueueMode {
        *self
            .steering_mode
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Replace the steering policy.
    pub fn set_steering_mode(&self, mode: QueueMode) {
        *self
            .steering_mode
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mode;
    }
    /// The current follow-up policy.
    pub fn follow_up_mode(&self) -> QueueMode {
        *self
            .follow_up_mode
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Replace the follow-up policy.
    pub fn set_follow_up_mode(&self, mode: QueueMode) {
        *self
            .follow_up_mode
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mode;
    }
    /// Store steering input without appending it to history.
    pub fn steer(&self, message: AgentMessage<C>) {
        self.steering
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(message);
    }
    /// Store follow-up input without appending it to history.
    pub fn follow_up(&self, message: AgentMessage<C>) {
        self.follow_up
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push_back(message);
    }
    /// Clear steering input, releasing retired entries after unlocking.
    pub fn clear_steering_queue(&self) {
        let retired = std::mem::take(
            &mut *self
                .steering
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        drop(retired);
    }
    /// Clear follow-up input, releasing retired entries after unlocking.
    pub fn clear_follow_up_queue(&self) {
        let retired = std::mem::take(
            &mut *self
                .follow_up
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
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
            let mut state = self
                .state
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        !self
            .steering
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
            || !self
                .follow_up
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
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
/// Private target-specific bounds for owned execution.
pub(crate) trait Native: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> Native for T {}
#[cfg(target_arch = "wasm32")]
/// Private target-specific bounds for owned execution.
pub(crate) trait Native {}
#[cfg(target_arch = "wasm32")]
impl<T> Native for T {}
