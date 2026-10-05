//! Owned transcript values and execution contracts.
use maestro_models::{AssistantMessage, Cancellation, Message, ModelEvent, ToolResultMessage};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};

/// A shared model record or open application-owned record.
#[derive(Clone, Debug, PartialEq)]
pub enum AgentMessage {
    /// A model conversation record.
    Model(Message),
    /// Opaque application data retained in raw history.
    Application {
        /// Caller-defined kind, not a closed vocabulary.
        kind: String,
        /// Caller-defined payload.
        data: Value,
        /// Supplied Unix-millisecond timestamp.
        timestamp: u64,
    },
}
/// Instructions separate from the authoritative transcript.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AgentContext {
    /// Current instructions.
    pub system_prompt: Option<String>,
    /// Ordered completed records.
    pub messages: Vec<AgentMessage>,
}
/// Detached transcript and runtime observation.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentState {
    /// Authoritative context.
    pub context: AgentContext,
    /// True through awaited end handlers.
    pub is_running: bool,
    /// Current independent cumulative assistant snapshot.
    pub streaming_message: Option<AssistantMessage>,
}
/// Ordered low-level execution events, not application settlement.
#[derive(Clone, Debug, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "message updates are the most frequent event and carry owned snapshots; boxing would allocate on every update"
)]
pub enum AgentEvent {
    /// A run begins.
    AgentStart,
    /// Run-local records after all turns.
    AgentEnd {
        /// New records only, excluding pre-existing history.
        messages: Vec<AgentMessage>,
    },
    /// A turn begins before its new input events.
    TurnStart,
    /// The authoritative completed turn.
    TurnEnd {
        /// Normalized terminal assistant.
        message: AssistantMessage,
        /// Empty in text-only execution.
        tool_results: Vec<ToolResultMessage>,
    },
    /// A new input or assistant begins.
    MessageStart {
        /// Independent starting record.
        message: AgentMessage,
    },
    /// A cumulative assistant update.
    MessageUpdate {
        /// Independent outer cumulative snapshot.
        message: AssistantMessage,
        /// Original owned model event, including its nested snapshot.
        assistant_message_event: ModelEvent,
    },
    /// An authoritative completed record.
    MessageEnd {
        /// Record appended to history before observation.
        message: AgentMessage,
    },
}
/// An ordered execution sink. Await accepted work; do not panic or await
/// this run's completion/idle. Detached work is not included in idle.
pub type AgentListener = Arc<
    dyn Fn(AgentEvent, Cancellation) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;
/// Initial context for an explicitly selected model.
#[derive(Clone, Default)]
pub struct AgentOptions {
    /// Initial instructions and authoritative history.
    pub context: AgentContext,
    /// Steering poll policy, defaulting independently to one record.
    pub steering_mode: QueueMode,
    /// Follow-up poll policy, defaulting independently to one record.
    pub follow_up_mode: QueueMode,
    /// Optional cooperative transformation of the owned message view before conversion.
    pub transform_context: Option<ContextTransform>,
    /// Optional infallible conversion into existing model records.
    pub convert_messages: Option<MessageConverter>,
    /// Optional boolean stop decision after awaited turn-end delivery.
    pub stop_after_turn: Option<StopAfterTurn>,
}
/// Admission failures or abnormal owned-task settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentError {
    /// Another direct run is active.
    Busy,
    /// Continuation has no history.
    NoUsableHistory,
    /// An assistant tail needs queued input.
    AssistantTail,
    /// No current Tokio runtime; admission has no effects.
    RuntimeUnavailable,
    /// Owned work failed abnormally, for example a callback panic.
    RunFailed,
}
impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Busy => "agent is busy",
            Self::NoUsableHistory => "continuation requires history",
            Self::AssistantTail => "assistant tail requires queued input",
            Self::RuntimeUnavailable => "agent requires a Tokio runtime",
            Self::RunFailed => "agent run failed",
        })
    }
}
impl std::error::Error for AgentError {}

/// Selects an independently controlled input FIFO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Queue {
    /// Input delivered at a turn boundary.
    Steering,
    /// Later input after steering is exhausted.
    FollowUp,
}

/// Transforms a detached message view before conversion; settle cooperatively
/// with the supplied cancellation and do not panic or await this run's idle.
pub type ContextTransform = Arc<
    dyn Fn(
            Vec<AgentMessage>,
            Cancellation,
        ) -> Pin<Box<dyn Future<Output = Vec<AgentMessage>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;
/// Converts or filters records without changing the raw transcript.
/// The converter is responsible for a usable model tail and must not panic.
pub type MessageConverter = Arc<
    dyn Fn(Vec<AgentMessage>) -> Pin<Box<dyn Future<Output = Vec<Message>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

/// Number of records consumed by one FIFO poll.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum QueueMode {
    /// Consume one record; the independent default for each queue.
    #[default]
    OneAtATime,
    /// Consume the current FIFO contents.
    All,
}

/// Completed turn and run-local records supplied after awaited turn-end sinks.
#[derive(Clone, Debug, PartialEq)]
pub struct StopAfterTurnContext {
    /// Authoritative completed assistant.
    pub message: AssistantMessage,
    /// Empty in text-only execution.
    pub tool_results: Vec<ToolResultMessage>,
    /// Current detached authoritative context.
    pub context: AgentContext,
    /// New records from this run only.
    pub new_messages: Vec<AgentMessage>,
}
/// Returns true to end before polling queues; false invents no continuation.
/// Must not panic or await this run's completion/idle.
pub type StopAfterTurn = Arc<
    dyn Fn(StopAfterTurnContext) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;
