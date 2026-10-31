//! Stored executable tool declarations.
use maestro_models::{DiagnosticErrorInfo, Tool};
/// Tool scheduling preference.
pub use maestro_tool::types::ToolExecutionMode;
use std::sync::RwLock;
/// Tool output with caller-defined optional details and termination.
pub type AgentToolResult<TDetails = serde_json::Value> =
    maestro_tool::types::AgentToolResult<Option<TDetails>, Option<bool>>;
/// A progress observer for tool output.
pub type AgentToolUpdateCallback<D = serde_json::Value, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Update<D>>;
/// Normalize retained arguments before validation.
pub type PrepareArguments<E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Prepare>;
/// Execute a tool with cancellation and progress.
pub type ExecuteTool<P = serde_json::Value, D = serde_json::Value, E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Execute<P, D>>;
/// Resolve a live model-facing description in the selected callable representation.
pub type ToolDescription<E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<<E as super::AgentExecution>::Description>;
/// An executable tool retained in conversation state.
pub struct AgentToolRecord<E: super::AgentExecution = super::Threaded> {
    /// Model-facing tool declaration.
    pub definition: Tool<E::SchemaCode>,
    /// Optional live description, resolved outside tool locks.
    pub description: Option<ToolDescription<E>>,
    /// Display label.
    pub label: String,
    /// Optional argument preparation.
    pub prepare_arguments: Option<PrepareArguments<E>>,
    /// Execution callback, retained without invocation by state operations.
    pub execute: ExecuteTool<serde_json::Value, serde_json::Value, E>,
    /// Optional scheduling preference.
    pub execution_mode: Option<ToolExecutionMode>,
}
/// A shared, mutable executable tool entry.
pub type SharedAgentTool<E = super::Threaded> =
    <E as maestro_models::Execution>::Shared<RwLock<AgentToolRecord<E>>>;

impl<E: super::AgentExecution> AgentToolRecord<E> {
    /// Materialize a declaration, preserving a supplied description failure.
    ///
    /// # Errors
    /// Returns the description callback's original diagnostic.
    pub fn resolve_definition(
        mut definition: Tool<E::SchemaCode>,
        description: Option<ToolDescription<E>>,
    ) -> Result<Tool<E::SchemaCode>, DiagnosticErrorInfo> {
        if let Some(description) = description {
            definition.description = description()?;
        }
        Ok(definition)
    }
    /// Wrap a typed callback in the common JSON tool interface.
    ///
    /// Arguments are decoded with [`serde_json::from_value`] and a decoding
    /// error does not invoke the callback. Details and progress details are
    /// encoded with [`serde_json::to_value`]; absent details stay absent and a
    /// present detail that encodes as JSON null stays an explicit null.
    #[must_use]
    pub fn typed<P: serde::de::DeserializeOwned + 'static, D: serde::Serialize + 'static>(
        definition: Tool<E::SchemaCode>,
        label: String,
        execute: ExecuteTool<P, D, E>,
    ) -> Self {
        Self {
            definition,
            description: None,
            label,
            prepare_arguments: None,
            execution_mode: None,
            execute: E::erase_execute(execute),
        }
    }
}
/// Encode typed final or partial details without changing content or termination.
pub(crate) fn encode_result<D: serde::Serialize>(
    result: AgentToolResult<D>,
) -> Result<AgentToolResult, DiagnosticErrorInfo> {
    Ok(AgentToolResult {
        content: result.content,
        details: result
            .details
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| json_error(&error))?,
        terminate: result.terminate,
        usage: None,
    })
}
/// Preserve the JSON boundary's failure text in the tool error channel.
pub(crate) fn json_error(error: &serde_json::Error) -> DiagnosticErrorInfo {
    maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(
        &error.to_string(),
    ))
}

/// Thread-safe default representation of the `AgentTool` record.
pub type AgentTool = AgentToolRecord<super::Threaded>;
/// Local representation of the `AgentTool` record.
pub type LocalAgentTool = AgentToolRecord<super::Local>;
