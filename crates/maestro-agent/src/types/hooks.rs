//! Tool callback records.
use super::{AgentContext, AgentMessage, AgentToolResult, CustomAgentMessages};
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, SharedAssistantMessage, ToolCall,
    ToolResultMessage, UserBlock,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Decision to block an admitted call; the default permits execution.
#[derive(Default)]
pub struct BeforeToolCallResult {
    /// Whether to block execution.
    pub block: bool,
    /// Exact nonempty blocking reason, or the default when absent or empty.
    pub reason: Option<String>,
}
/// Requesting records and mutable validated arguments.
pub struct BeforeToolCallContext<'a, C: CustomAgentMessages = crate::NoCustomMessages> {
    /// Assistant requesting this call.
    pub assistant_message: &'a SharedAssistantMessage,
    /// Original model-supplied call.
    pub tool_call: &'a ToolCall,
    /// Validated arguments, editable without another schema check.
    pub args: &'a mut serde_json::Value,
    /// Current conversation inputs.
    pub context: &'a AgentContext<C>,
}
/// Inspect or edit validated arguments before execution.
#[cfg(not(target_arch = "wasm32"))]
pub type BeforeToolCall<C> = Arc<
    dyn for<'a> Fn(
            BeforeToolCallContext<'a, C>,
            Option<Cancellation>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<BeforeToolCallResult>, DiagnosticErrorInfo>>
                    + Send
                    + 'a,
            >,
        > + Send
        + Sync,
>;
/// Inspect or edit validated arguments in a browser-local callback.
#[cfg(target_arch = "wasm32")]
pub type BeforeToolCall<C> = Arc<
    dyn for<'a> Fn(
        BeforeToolCallContext<'a, C>,
        Option<Cancellation>,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<BeforeToolCallResult>, DiagnosticErrorInfo>> + 'a>,
    >,
>;

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
pub struct AfterToolCallContext<'a, C: CustomAgentMessages = crate::NoCustomMessages> {
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
    pub context: &'a AgentContext<C>,
}
/// Select final result fields after execution and admitted progress settle.
#[cfg(not(target_arch = "wasm32"))]
pub type AfterToolCall<C> = Arc<
    dyn for<'a> Fn(
            AfterToolCallContext<'a, C>,
            Option<Cancellation>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<AfterToolCallResult>, DiagnosticErrorInfo>>
                    + Send
                    + 'a,
            >,
        > + Send
        + Sync,
>;
/// Select final result fields in a browser-local callback.
#[cfg(target_arch = "wasm32")]
pub type AfterToolCall<C> = Arc<
    dyn for<'a> Fn(
        AfterToolCallContext<'a, C>,
        Option<Cancellation>,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<AfterToolCallResult>, DiagnosticErrorInfo>> + 'a>,
    >,
>;

/// Completed turn and newly produced operation records.
pub struct ShouldStopAfterTurnContext<'a, C: CustomAgentMessages = crate::NoCustomMessages> {
    /// Completed assistant response.
    pub message: &'a SharedAssistantMessage,
    /// Tool artifacts for this turn only.
    pub tool_results: &'a [ToolResultMessage],
    /// Current conversation with completed results appended.
    pub context: &'a AgentContext<C>,
    /// Entries produced by this operation, including prompts when supplied.
    pub new_messages: &'a [AgentMessage<C>],
}
/// Decide whether to stop after the completed turn observation.
#[cfg(not(target_arch = "wasm32"))]
pub type ShouldStopAfterTurn<C> = Arc<
    dyn for<'a> Fn(
            ShouldStopAfterTurnContext<'a, C>,
        )
            -> Pin<Box<dyn Future<Output = Result<bool, DiagnosticErrorInfo>> + Send + 'a>>
        + Send
        + Sync,
>;
/// Decide whether to stop in a browser-local callback.
#[cfg(target_arch = "wasm32")]
pub type ShouldStopAfterTurn<C> = Arc<
    dyn for<'a> Fn(
        ShouldStopAfterTurnContext<'a, C>,
    ) -> Pin<Box<dyn Future<Output = Result<bool, DiagnosticErrorInfo>> + 'a>>,
>;
/// Poll ordered steering entries at the loop's turn boundaries.
#[cfg(not(target_arch = "wasm32"))]
pub type GetSteeringMessages<C> =
    Arc<dyn Fn() -> BoxFuture<Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo>> + Send + Sync>;
/// Poll browser-local ordered steering entries at turn boundaries.
#[cfg(target_arch = "wasm32")]
pub type GetSteeringMessages<C> =
    Arc<dyn Fn() -> BoxFuture<Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo>>>;
/// Poll ordered follow-ups when tools and steering are exhausted.
pub type GetFollowUpMessages<C> = GetSteeringMessages<C>;
