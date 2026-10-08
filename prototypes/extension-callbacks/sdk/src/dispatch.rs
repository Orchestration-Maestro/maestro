//! Inbound dispatch both adapters share: running a registered callback and shaping its result.

use crate::api::{ExtensionResult, InputHandler};
use crate::bindings::maestro::extension::types::{InputEvent, InputOutcome, ToolResult};
use crate::context::{AbortSignal, ExtensionContext};
use crate::tool::{AgentToolUpdateCallback, PrepareArguments, ToolExecute};

/// Runs an input handler; the event is returned as the handler left it, even when it failed.
pub async fn run_input(
    handler: &InputHandler,
    mut event: InputEvent,
    ctx: ExtensionContext,
) -> InputOutcome {
    let decision = handler(&mut event, ctx).await;
    InputOutcome { event, decision }
}

/// Rewrites raw JSON arguments through the tool's preparation callback.
///
/// # Errors
/// Returns the JSON or callback message.
pub fn prepare_arguments(prepare: &PrepareArguments, raw: &str) -> ExtensionResult<String> {
    let value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    prepare(value).map(|prepared| prepared.to_string())
}

/// The host-supplied parts of one tool call.
pub struct ToolInvocation {
    /// Call identifier.
    pub call_id: String,
    /// Prepared arguments as JSON text.
    pub params: String,
    /// Retained cancellation flag.
    pub signal: Option<AbortSignal>,
    /// Progress callback.
    pub update: Option<AgentToolUpdateCallback>,
}

/// Runs a tool body and converts its result to the wire record.
///
/// # Errors
/// Returns the JSON or tool message.
pub async fn run_tool(
    execute: &ToolExecute,
    call: ToolInvocation,
    ctx: ExtensionContext,
) -> ExtensionResult<ToolResult> {
    let params = serde_json::from_str(&call.params).map_err(|error| error.to_string())?;
    let result = execute(call.call_id, params, call.signal, call.update, ctx).await?;
    Ok(result.into())
}
