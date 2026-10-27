//! Supplied bash tool data.
use super::truncate::TruncationResult;
use crate::{AbortSignal, ExtensionFuture, ExtensionResult, Presence};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the bash tool.
pub struct BashToolInput {
    /// Command.
    pub command: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Timeout.
    pub timeout: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the bash tool.
pub struct BashToolDetails {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Truncation.
    pub truncation: Presence<TruncationResult>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Full output path.
    pub full_output_path: Presence<String>,
}

/// Output callback receiving an uninterpreted byte chunk.
pub type BashOnData = Rc<dyn Fn(&[u8]) -> ExtensionResult<()>>;
/// Owned operands of one custom execution.
#[derive(Clone)]
pub struct BashExecOptions {
    /// Output receiver.
    pub on_data: BashOnData,
    /// Supplied cancellation flag.
    pub signal: Option<AbortSignal>,
    /// Supplied timeout in seconds, without scheduling policy.
    pub timeout: Option<f64>,
    /// Supplied environment, with absent values retained.
    pub env: Option<IndexMap<String, Option<String>>>,
}
/// Outcome of an authored execution, with an optional numeric exit code.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BashExecResult {
    /// Supplied process exit code.
    pub exit_code: Option<f64>,
}
/// Asynchronous execution supplied by an extension.
pub type BashExec =
    Rc<dyn Fn(String, String, BashExecOptions) -> ExtensionFuture<'static, BashExecResult>>;
/// Pluggable command execution with independently retained author state.
#[derive(Clone)]
pub struct BashOperations {
    /// Executes a command with its supplied working directory and options.
    pub exec: BashExec,
}
impl std::fmt::Debug for BashOperations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BashOperations").finish_non_exhaustive()
    }
}

/// Owned shell operands observed after command prefixing.
#[derive(Clone, Serialize, Deserialize)]
pub struct BashSpawnContext {
    /// Literal command, including any configured prefix.
    pub command: String,
    /// Selected execution directory.
    pub cwd: String,
    /// Complete replacement environment, retaining unset entries.
    pub env: IndexMap<String, Option<String>>,
}
/// Synchronous operand replacement with callback-scoped event emission.
pub type BashSpawnHook = Rc<
    dyn for<'a> Fn(
        BashSpawnContext,
        crate::CallbackEmitter<'a>,
    ) -> ExtensionResult<BashSpawnContext>,
>;
/// Independent controls for the original native shell factory.
#[derive(Default)]
pub struct BashToolOptions {
    /// Replacement execution; omission selects the native local shell.
    pub operations: Option<BashOperations>,
    /// Nonempty prefix inserted before the original command.
    pub command_prefix: Option<String>,
    /// Explicit local executable, ignored when operations are supplied.
    pub shell_path: Option<String>,
    /// Synchronous interception after prefixing and before execution.
    pub spawn_hook: Option<BashSpawnHook>,
}
