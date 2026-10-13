//! Stored executable tool declarations.
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, Tool, UserBlock};
use std::sync::{Arc, RwLock};
/// Tool scheduling preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolExecutionMode {
    /// Execute without overlapping other tools.
    Sequential,
    /// Permit concurrent execution.
    Parallel,
}
/// Tool output with caller-defined details.
///
/// `details` has three states: `None` is absent and is omitted from the tool
/// result message, `Some` of a JSON null keeps an explicit null, and any other
/// `Some` carries its value.
pub struct AgentToolResult<TDetails = serde_json::Value> {
    /// Text and image output.
    pub content: Vec<UserBlock>,
    /// Caller-defined result data; `None` when the tool returned none.
    pub details: Option<TDetails>,
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
/// Normalize retained raw arguments in place and return arguments for schema validation.
#[cfg(not(target_arch = "wasm32"))]
pub type PrepareArguments = Arc<
    dyn Fn(&mut serde_json::Value) -> Result<serde_json::Value, DiagnosticErrorInfo> + Send + Sync,
>;
/// Normalize retained browser arguments in place and return arguments for validation.
#[cfg(target_arch = "wasm32")]
pub type PrepareArguments =
    Arc<dyn Fn(&mut serde_json::Value) -> Result<serde_json::Value, DiagnosticErrorInfo>>;
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
pub struct AgentTool {
    /// Model-facing tool declaration.
    pub definition: Tool,
    /// Display label.
    pub label: String,
    /// Optional argument preparation.
    pub prepare_arguments: Option<PrepareArguments>,
    /// Execution callback, retained without invocation by state operations.
    pub execute: ExecuteTool,
    /// Optional scheduling preference.
    pub execution_mode: Option<ToolExecutionMode>,
}
/// A shared, mutable executable tool entry.
pub type SharedAgentTool = Arc<RwLock<AgentTool>>;

impl AgentTool {
    /// Wrap a typed callback in the common JSON tool interface.
    ///
    /// Arguments are decoded with [`serde_json::from_value`] and a decoding
    /// error does not invoke the callback. Details and progress details are
    /// encoded with [`serde_json::to_value`]; absent details stay absent and a
    /// present detail that encodes as JSON null stays an explicit null.
    #[must_use]
    pub fn typed<P: serde::de::DeserializeOwned + 'static, D: serde::Serialize + 'static>(
        definition: Tool,
        label: String,
        execute: ExecuteTool<P, D>,
    ) -> Self {
        Self {
            definition,
            label,
            prepare_arguments: None,
            execution_mode: None,
            execute: erase_execute(execute),
        }
    }
}
/// Adapt the typed callback while keeping the retained record independent of its types.
fn erase_execute<P: serde::de::DeserializeOwned + 'static, D: serde::Serialize + 'static>(
    execute: ExecuteTool<P, D>,
) -> ExecuteTool {
    Arc::new(move |id, args, signal, update| {
        let parameters = match serde_json::from_value(args) {
            Ok(parameters) => parameters,
            Err(error) => return Box::pin(async move { Err(json_error(&error)) }),
        };
        let failure = Arc::new(RwLock::new(None));
        let progress_failure = Arc::clone(&failure);
        let progress = update.map(|update| {
            Arc::new(move |result| match encode_result(result) {
                Ok(result) => update(result),
                Err(error) => {
                    *progress_failure
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(error);
                }
            }) as AgentToolUpdateCallback<D>
        });
        let result = execute(id, parameters, signal, progress);
        Box::pin(async move {
            let result = result.await?;
            if let Some(error) = failure
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
            {
                return Err(error);
            }
            encode_result(result)
        })
    })
}
/// Encode typed final or partial details without changing content or termination.
fn encode_result<D: serde::Serialize>(
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
    })
}
/// Preserve the JSON boundary's failure text in the tool error channel.
fn json_error(error: &serde_json::Error) -> DiagnosticErrorInfo {
    maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(
        &error.to_string(),
    ))
}
