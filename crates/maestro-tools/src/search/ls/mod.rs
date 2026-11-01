//! The `ls` tool: directory listings through replaceable operations.

mod execute;
mod render;

use crate::{
    output::truncate::{DEFAULT_MAX_BYTES, TruncationResult},
    tool_definition_wrapper::wrap_tool_definition,
    types::ToolDefinition,
};
use execute::{Lister, NativeLsOperations};
use maestro_agent::AgentTool;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// Entries listed when no limit is given.
const DEFAULT_LIMIT: f64 = 500.0;

/// Arguments of one listing.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct LsToolInput {
    /// Directory to list; absent or empty means the working directory.
    pub path: Option<String>,
    /// Maximum number of entries to return.
    pub limit: Option<f64>,
}

/// What a listing reports beyond its text.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LsToolDetails {
    /// The byte truncation applied to the listing, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<TruncationResult>,
    /// The entry limit that stopped the listing, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_limit_reached: Option<f64>,
}

/// The file access a listing performs; replace it to list through another system.
pub trait LsOperations: Send + Sync {
    /// Whether the path exists.
    fn exists(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>>;

    /// Whether the path is a directory; fails when it cannot be examined.
    fn stat(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>>;

    /// The names of the directory's entries.
    fn readdir(&self, absolute_path: String)
    -> BoxFuture<Result<Vec<String>, DiagnosticErrorInfo>>;
}

/// Options of the ls tool.
#[derive(Default)]
pub struct LsToolOptions {
    /// File access to use; `None` lists the local filesystem.
    pub operations: Option<Arc<dyn LsOperations>>,
}

/// One factory body with concrete callback ownership.
macro_rules! factories {
 ($ptr:ident, [$($bound:tt)*]) => {
/// Build the `ls` tool definition.
///
/// Paths are resolved against `cwd` when the tool is called, not here.
#[must_use]
pub fn create_ls_tool_definition<C: 'static $($bound)*>(
    cwd: String,
    options: LsToolOptions,
) -> ToolDefinition<C, LsToolInput, LsToolDetails> {
    let lister = Arc::new(Lister {
        cwd,
        operations: options
            .operations
            .unwrap_or_else(|| Arc::new(NativeLsOperations)),
    });
    let description = format!(
        "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to {DEFAULT_LIMIT} entries or {}KB (whichever is hit first).",
        DEFAULT_MAX_BYTES / 1024
    );
    ToolDefinition {
        definition: Tool {
            name: "ls".into(),
            description,
            parameters: maestro_models::ParameterSchema::authored(json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory to list (default: current directory)"},
                    "limit": {"type": "number", "description": "Maximum number of entries to return (default: 500)"},
                },
            }), [("".to_owned(), maestro_models::SchemaKind::Object), ("/properties/path".to_owned(), maestro_models::SchemaKind::String), ("/properties/limit".to_owned(), maestro_models::SchemaKind::Number)]),
        },
        label: "ls".into(),
        prompt_snippet: Some("List directory contents".into()),
        prompt_guidelines: None,
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: $ptr::new(move |_id, input, signal, _update, _context| {
            let lister = Arc::clone(&lister);
            Box::pin(async move { lister.list(input, signal).await })
        }),
        render_call: Some($ptr::new(render::render_call)),
        render_result: Some(crate::ToolRenderResult::Raw($ptr::new(render::render_result) as _)),
    }
}

/// Build the executable `ls` tool with no caller context.
#[must_use]
pub fn create_ls_tool(cwd: String, options: LsToolOptions) -> AgentTool {
    wrap_tool_definition(create_ls_tool_definition::<()>(cwd, options), None)
}

 };
}
factories!(Arc, [+ Send]);
/// Caller-local factories over the same file operations.
pub mod local {
    use super::{
        Arc, DEFAULT_LIMIT, DEFAULT_MAX_BYTES, Lister, LsToolDetails, LsToolInput, LsToolOptions,
        NativeLsOperations, Tool, json, render,
    };
    use crate::tool_definition_wrapper::local::wrap_tool_definition;
    use crate::types::local::ToolDefinition;
    use std::rc::Rc;
    /// Caller-local executable tool.
    type AgentTool = maestro_agent::AgentToolRecord<maestro_models::Local>;
    factories!(Rc, []);
}
