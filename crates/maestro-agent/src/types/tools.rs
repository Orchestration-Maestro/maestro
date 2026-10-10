//! Stored executable tool declarations.
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, Tool, UserBlock};
use std::sync::Arc;
/// Tool scheduling preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolExecutionMode {
    /// Execute without overlapping other tools.
    Sequential,
    /// Permit concurrent execution.
    Parallel,
}
/// Tool output with caller-defined details.
pub struct AgentToolResult<TDetails = serde_json::Value> {
    /// Text and image output.
    pub content: Vec<UserBlock>,
    /// Caller-defined result data.
    pub details: TDetails,
    /// Whether this result requests termination.
    pub terminate: Option<bool>,
}
/// A progress observer for tool output.
#[cfg(not(target_arch = "wasm32"))]
pub type AgentToolUpdateCallback<TDetails = serde_json::Value> =
    Arc<dyn Fn(AgentToolResult<TDetails>) + Send + Sync>;
/// A browser progress observer for tool output.
#[cfg(target_arch = "wasm32")]
pub type AgentToolUpdateCallback<TDetails = serde_json::Value> =
    Arc<dyn Fn(AgentToolResult<TDetails>)>;
/// Convert raw arguments into the tool's parameter type.
#[cfg(not(target_arch = "wasm32"))]
pub type PrepareArguments<TParameters = serde_json::Value> =
    Arc<dyn Fn(serde_json::Value) -> Result<TParameters, DiagnosticErrorInfo> + Send + Sync>;
/// Convert raw arguments into the browser tool's parameter type.
#[cfg(target_arch = "wasm32")]
pub type PrepareArguments<TParameters = serde_json::Value> =
    Arc<dyn Fn(serde_json::Value) -> Result<TParameters, DiagnosticErrorInfo>>;
/// Execute a tool with its call id, arguments, cancellation and progress observer.
#[cfg(not(target_arch = "wasm32"))]
pub type ExecuteTool<TParameters = serde_json::Value, TDetails = serde_json::Value> = Arc<
    dyn Fn(
            String,
            TParameters,
            Option<Cancellation>,
            Option<AgentToolUpdateCallback<TDetails>>,
        ) -> BoxFuture<Result<AgentToolResult<TDetails>, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;
/// Execute a browser tool with its call id, arguments, cancellation and progress observer.
#[cfg(target_arch = "wasm32")]
pub type ExecuteTool<TParameters = serde_json::Value, TDetails = serde_json::Value> = Arc<
    dyn Fn(
        String,
        TParameters,
        Option<Cancellation>,
        Option<AgentToolUpdateCallback<TDetails>>,
    ) -> BoxFuture<Result<AgentToolResult<TDetails>, DiagnosticErrorInfo>>,
>;
/// An executable tool retained in conversation state.
pub struct AgentTool<TParameters = serde_json::Value, TDetails = serde_json::Value> {
    /// Model-facing tool declaration.
    pub definition: Tool,
    /// Display label.
    pub label: String,
    /// Optional argument preparation.
    pub prepare_arguments: Option<PrepareArguments<TParameters>>,
    /// Execution callback, retained without invocation by state operations.
    pub execute: ExecuteTool<TParameters, TDetails>,
    /// Optional scheduling preference.
    pub execution_mode: Option<ToolExecutionMode>,
}
