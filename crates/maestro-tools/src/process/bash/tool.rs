//! Agent shell records and factories.

use super::{BashOperations, create_local_bash_operations};
use crate::{
    ToolDefinition, TruncationResult, tool_definition_wrapper::wrap_tool_definition,
    utils::shell::ShellEnv,
};
use maestro_agent::{AgentTool, AgentToolResult};
use maestro_models::{DiagnosticErrorInfo, Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// Arguments of an agent shell call.
#[derive(Deserialize)]
pub struct BashToolInput {
    /// Literal shell command.
    pub command: String,
    /// Optional timeout in seconds, forwarded to the operation.
    pub timeout: Option<f64>,
}

/// Output selection and artifact identity.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BashToolDetails {
    /// Display truncation, when selected output exceeds a limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<TruncationResult>,
    /// Raw-output artifact, when one was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_output_path: Option<String>,
}

/// Command operands replaced as a unit by a spawn hook.
pub struct BashSpawnContext {
    /// Literal command including any configured prefix.
    pub command: String,
    /// Execution directory.
    pub cwd: String,
    /// Complete replacement environment.
    pub env: ShellEnv,
}

/// Replaces command operands or fails before execution starts.
pub type BashSpawnHook =
    Arc<dyn Fn(BashSpawnContext) -> Result<BashSpawnContext, DiagnosticErrorInfo> + Send + Sync>;

/// Selects the shell operation and command preparation.
#[derive(Default)]
pub struct BashToolOptions {
    /// Supplied operation; otherwise uses the local shell.
    pub operations: Option<Arc<dyn BashOperations>>,
    /// Nonempty literal prefix, followed by one newline.
    pub command_prefix: Option<String>,
    /// Explicit executable for local operations only.
    pub shell_path: Option<String>,
    /// Operand replacement after prefix insertion.
    pub spawn_hook: Option<BashSpawnHook>,
}

/// Build the native shell definition with call/result renderers and shared timing state.
///
/// The caller context is unused; [`super::BashRenderState`] owns row refresh teardown.
#[must_use]
pub fn create_bash_tool_definition<C: 'static>(
    cwd: String,
    bin_dir: String,
    options: BashToolOptions,
) -> ToolDefinition<C, BashToolInput, BashToolDetails, super::BashRenderState> {
    let operations = options
        .operations
        .unwrap_or_else(|| create_local_bash_operations(bin_dir.clone(), options.shell_path));
    ToolDefinition {
        definition: Tool {
            name: "bash".into(),
            description: "Execute a bash command in the current working directory. Returns stdout and stderr. Output is truncated to last 2000 lines or 50KB (whichever is hit first). If truncated, full output is saved to a temp file. Optionally provide a timeout in seconds.".into(),
            parameters: json!({"type": "object", "required": ["command"], "properties": {
                "command": {"type": "string", "description": "Bash command to execute"},
                "timeout": {"type": "number", "description": "Timeout in seconds (optional, no default timeout)"}
            }}),
        },
        label: "bash".into(),
        prompt_snippet: Some("Execute bash commands (ls, grep, find, etc.)".into()),
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_, input, signal, update, _| {
            let command = match options.command_prefix.as_deref().filter(|prefix| !prefix.is_empty()) {
                Some(prefix) => format!("{prefix}\n{}", input.command),
                None => input.command,
            };
            let context = BashSpawnContext { command, cwd: cwd.clone(), env: crate::utils::shell::get_shell_env(&bin_dir) };
            let context = if let Some(hook) = &options.spawn_hook {
                match hook(context) {
                    Ok(context) => context,
                    Err(error) => return Box::pin(std::future::ready(Err(error))),
                }
            } else { context };
            if let Some(update) = &update { update(AgentToolResult { content: Vec::new(), details: None, terminate: None }); }
            let operations = Arc::clone(&operations);
            Box::pin(super::execute::run(operations, context, input.timeout, signal, update))
        }),
        render_call: Some(std::rc::Rc::new(super::render::render_call)),
        render_result: Some(std::rc::Rc::new(super::render::render_result)),
    }
}

/// Wrap the native shell definition as an executable agent tool.
#[must_use]
pub fn create_bash_tool(cwd: String, bin_dir: String, options: BashToolOptions) -> AgentTool {
    wrap_tool_definition(
        create_bash_tool_definition::<()>(cwd, bin_dir, options),
        None,
    )
}
