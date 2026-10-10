//! Authored tool definitions and registration metadata.
use crate::Presence;
use crate::{
    AbortSignal, AgentToolResult, AgentToolUpdateCallback, ExtensionContext, ExtensionFuture,
    ExtensionResult, ToolExecutionMode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::rc::Rc;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Authored shell rendering choice.
pub enum RenderShell {
    /// Host-provided shell rendering.
    Default,
    #[serde(rename = "self")]
    /// Tool-provided shell rendering.
    SelfRendered,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Authored registration metadata.
pub struct ToolMetadata {
    /// Name.
    pub name: String,
    /// Label.
    pub label: String,
    /// Description.
    pub description: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Prompt snippet.
    pub prompt_snippet: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Prompt guidelines.
    pub prompt_guidelines: Presence<Vec<String>>,
    /// Parameters.
    pub parameters: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Render shell.
    pub render_shell: Presence<RenderShell>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Execution mode.
    pub execution_mode: Presence<ToolExecutionMode>,
}
/// Synchronous transformation of invocation arguments.
pub type PrepareArguments = Rc<dyn Fn(Value) -> ExtensionResult<Value>>;
/// Asynchronous execution receiving owned context, signal and update handles.
pub type ToolExecute = Rc<
    dyn Fn(
        String,
        Value,
        Option<AbortSignal>,
        Option<AgentToolUpdateCallback>,
        ExtensionContext,
    ) -> ExtensionFuture<'static, AgentToolResult>,
>;
/// A registered tool with optional preparation and asynchronous execution.
pub struct ToolDefinition {
    /// Metadata.
    pub metadata: ToolMetadata,
    /// Prepare arguments.
    pub prepare_arguments: Option<PrepareArguments>,
    /// Execute.
    pub execute: ToolExecute,
}
