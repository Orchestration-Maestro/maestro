//! Tool callback records.
use super::{AgentContextRecord, AgentMessage, AgentToolResult, CustomAgentMessages};
use maestro_models::{SharedAssistantMessage, ToolCall, ToolResultMessage, UserBlock};

/// Decision to block an admitted call; the default permits execution.
#[derive(Default)]
pub struct BeforeToolCallResult {
    /// Whether to block execution.
    pub block: bool,
    /// Exact nonempty blocking reason, or the default when absent or empty.
    pub reason: Option<String>,
}
/// Requesting records and mutable validated arguments.
pub struct BeforeToolCallContext<
    'a,
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Assistant requesting this call.
    pub assistant_message: &'a SharedAssistantMessage,
    /// Original model-supplied call.
    pub tool_call: &'a ToolCall,
    /// Validated arguments, editable without another schema check.
    pub args: &'a mut serde_json::Value,
    /// Current conversation inputs.
    pub context: &'a AgentContextRecord<C, E>,
}
/// Retained `BeforeToolCall` callback.
pub type BeforeToolCall<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::BeforeToolCall<C>>;

/// Optional whole-field replacements after execution; default retains originals.
#[derive(Default)]
pub struct AfterToolCallResult {
    /// Replacement content, including an empty vector.
    pub content: Option<Vec<UserBlock>>,
    /// Replacement details; absent or JSON null retains the original.
    pub details: Option<serde_json::Value>,
    /// Replacement error flag, including false.
    pub is_error: Option<bool>,
    /// Replacement continuation hint, including false.
    pub terminate: Option<bool>,
}
/// Executed outcome and requesting records borrowed for finalization.
pub struct AfterToolCallContext<
    'a,
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Assistant requesting this call.
    pub assistant_message: &'a SharedAssistantMessage,
    /// Original model-supplied call.
    pub tool_call: &'a ToolCall,
    /// Arguments passed to execution after before-hook edits.
    pub args: &'a serde_json::Value,
    /// Execution result, including an artifact for execution failure.
    pub result: &'a AgentToolResult,
    /// Whether execution failed.
    pub is_error: bool,
    /// Current conversation inputs.
    pub context: &'a AgentContextRecord<C, E>,
}
/// Retained `AfterToolCall` callback.
pub type AfterToolCall<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::AfterToolCall<C>>;

/// Completed turn and newly produced operation records.
pub struct ShouldStopAfterTurnContext<
    'a,
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Completed assistant response.
    pub message: &'a SharedAssistantMessage,
    /// Tool artifacts for this turn only.
    pub tool_results: &'a [ToolResultMessage],
    /// Current conversation with completed results appended.
    pub context: &'a AgentContextRecord<C, E>,
    /// Entries produced by this operation, including prompts when supplied.
    pub new_messages: &'a [AgentMessage<C>],
}
/// The completed turn the next-turn preparation reads.
pub type PrepareNextTurnContext<'a, C, E = super::Threaded> = ShouldStopAfterTurnContext<'a, C, E>;
/// Replacement runtime state for the next provider request; absent parts keep the current
/// ones. The request already sent is never changed.
pub struct AgentLoopTurnUpdate<
    C: CustomAgentMessages = crate::NoCustomMessages,
    E: super::AgentExecution = super::Threaded,
> {
    /// Instructions, history and tools for the next request.
    pub context: Option<AgentContextRecord<C, E>>,
    /// Model for the next request.
    pub model: Option<std::sync::Arc<std::sync::RwLock<maestro_models::Model>>>,
    /// Thinking level for the next request.
    pub thinking_level: Option<maestro_models::ModelThinkingLevel>,
}
/// Retained `PrepareNextTurn` callback.
pub type PrepareNextTurn<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::PrepareNextTurn<C>>;
/// Completed-turn stop callback.
pub type ShouldStopAfterTurn<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Stop<C>>;
/// Ordered steering poll.
pub type GetSteeringMessages<C, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Poll<C>>;
/// Ordered follow-up poll.
pub type GetFollowUpMessages<C, E = super::Threaded> = GetSteeringMessages<C, E>;
