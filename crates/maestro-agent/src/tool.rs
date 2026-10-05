//! Executable tool callbacks and ordered interception contracts.
use crate::AgentContext;
use maestro_models::{AssistantMessage, Cancellation, InputContent, ToolCall, ToolDeclaration};
use serde_json::{Map, Value};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
/// Batch scheduling; parallel is the default.
pub enum ToolExecutionMode {
    #[default]
    /// Preflight serially, then execute concurrently.
    Parallel,
    /// Settle each complete call before starting the next.
    Sequential,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Owned text/image output with arbitrary details and a runtime-only termination hint.
pub struct ToolResult {
    /// Ordered text/image blocks.
    pub content: Vec<InputContent>,
    /// Arbitrary JSON details; supplied null replaces prior details.
    pub details: Value,
    /// Runtime-only batch termination hint, not transcript data.
    pub terminate: Option<bool>,
}

/// Synchronous owned progress submission with independent delivery for each update.
/// Subscribers run in registration order within each update. All deliveries are
/// awaited before finalization, even when execution fails.
/// This callback may be called from any thread during the tool's execution.
/// Do not retain or use this callback after the execution future settles.
pub type ToolProgress = Arc<dyn Fn(ToolResult) + Send + Sync + 'static>;

#[derive(Clone)]
/// Owned execution inputs with shared cooperative cancellation.
pub struct ToolInvocation {
    /// Original call identity.
    pub tool_call_id: String,
    /// Current whole argument object.
    pub args: Map<String, Value>,
    /// Shared cooperative run cancellation.
    pub cancellation: Cancellation,
    /// Submit owned partial output during execution.
    pub progress: ToolProgress,
}

/// Cancellable current-format preparation before shared validation. Settle cooperatively.
pub type ToolPrepare = Arc<
    dyn Fn(
            Map<String, Value>,
            Cancellation,
        )
            -> Pin<Box<dyn Future<Output = Result<Map<String, Value>, String>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

/// Execute once; expected failures return diagnostic text. Settle cooperatively.
pub type ToolExecute = Arc<
    dyn Fn(
            ToolInvocation,
        ) -> Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone)]
/// Executable behavior attached to an existing model declaration.
pub struct Tool {
    /// Shared model-facing declaration.
    pub declaration: ToolDeclaration,
    /// Readable tool label.
    pub label: String,
    /// Optional preparation before shared validation.
    pub prepare: Option<ToolPrepare>,
    /// Injected execution callback.
    pub execute: ToolExecute,
    /// Optional whole-batch scheduling override.
    pub execution_mode: Option<ToolExecutionMode>,
}

#[derive(Clone, Debug, PartialEq)]
/// Detached raw request and current working input at batch entry.
pub struct BeforeToolCallContext {
    /// Unmodified completed assistant snapshot.
    pub assistant_message: AssistantMessage,
    /// Unmodified original call.
    pub tool_call: ToolCall,
    /// Current whole argument object.
    pub args: Map<String, Value>,
    /// Detached assistant-completed batch context, excluding this batch results.
    pub context: AgentContext,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Whole argument replacement or a block; blocking wins over replacement.
pub struct BeforeToolCallResult {
    /// Current whole argument object.
    pub args: Option<Map<String, Value>>,
    /// Reject execution without running later hooks.
    pub block: bool,
    /// Nonempty rejection text; absent or empty uses the default reason.
    pub reason: Option<String>,
}

/// Ordered cancellable preflight hook. Replacements are not revalidated.
pub type BeforeToolCall = Arc<
    dyn Fn(
            BeforeToolCallContext,
            Cancellation,
        )
            -> Pin<Box<dyn Future<Output = Result<BeforeToolCallResult, String>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Debug, PartialEq)]
/// Detached request with accumulated executed outcome.
pub struct AfterToolCallContext {
    /// Unmodified completed assistant snapshot.
    pub assistant_message: AssistantMessage,
    /// Unmodified original call.
    pub tool_call: ToolCall,
    /// Current whole argument object.
    pub args: Map<String, Value>,
    /// Current accumulated tool output.
    pub result: ToolResult,
    /// Separate error flag; supplied false clears a prior error.
    pub is_error: bool,
    /// Detached assistant-completed batch context, excluding this batch results.
    pub context: AgentContext,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Supplied whole-field overrides; omission preserves prior values.
pub struct AfterToolCallResult {
    /// Ordered text/image blocks.
    pub content: Option<Vec<InputContent>>,
    /// Arbitrary JSON details; supplied null replaces prior details.
    pub details: Option<Value>,
    /// Separate error flag; supplied false clears a prior error.
    pub is_error: Option<bool>,
    /// Runtime-only batch termination hint, not transcript data.
    pub terminate: Option<bool>,
}

/// Ordered cancellable finalizer on executed success or failure.
pub type AfterToolCall = Arc<
    dyn Fn(
            AfterToolCallContext,
            Cancellation,
        )
            -> Pin<Box<dyn Future<Output = Result<AfterToolCallResult, String>> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;
