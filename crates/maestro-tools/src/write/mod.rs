//! The `write` tool: complete file contents written through replaceable operations.

mod execute;
mod highlight;
mod render;

use crate::{
    file_mutation_queue::with_file_mutation_queue, path_utils::resolve_existing_path,
    tool_definition_wrapper::wrap_tool_definition, types::ToolDefinition,
    utils::diagnostics::diagnostic,
};
pub(crate) use execute::NativeWriteOperations;
use execute::write_file;
use maestro_agent::AgentTool;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Tool};
use maestro_path::dirname;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Arguments of one write.
#[derive(Clone, Debug, Deserialize)]
pub struct WriteToolInput {
    /// Path to the file, relative or absolute.
    pub path: String,
    /// Content that replaces the file.
    pub content: String,
}

/// The file access a write performs; replace it to write through another system.
pub trait WriteOperations: Send + Sync {
    /// Write `content` to the file as UTF-8.
    fn write_file(
        &self,
        absolute_path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;

    /// Create the directory and any missing parents.
    fn mkdir(&self, dir: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;
}

/// Options of the write tool.
#[derive(Default)]
pub struct WriteToolOptions {
    /// File access to use; `None` writes the local filesystem.
    pub operations: Option<Arc<dyn WriteOperations>>,
}

/// One factory body with concrete callback ownership.
macro_rules! factories {
 ($ptr:ident, [$($bound:tt)*]) => {
/// Build the `write` tool definition.
///
/// The path is resolved against `cwd` when the tool is called, before any asynchronous
/// work, so the returned future is unaffected by later working directory changes.
#[must_use]
pub fn create_write_tool_definition<C: 'static>(
    cwd: String,
    options: WriteToolOptions,
) -> ToolDefinition<C, WriteToolInput, ()> {
    let operations = options
        .operations
        .unwrap_or_else(|| Arc::new(NativeWriteOperations));
    ToolDefinition {
        definition: Tool {
            name: "write".into(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".into(),
            parameters: maestro_models::ParameterSchema::authored(json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": {"type": "string", "description": "Path to the file to write (relative or absolute)"},
                    "content": {"type": "string", "description": "Content to write to the file"},
                },
            }), [("".to_owned(), maestro_models::SchemaKind::Object), ("/properties/path".to_owned(), maestro_models::SchemaKind::String), ("/properties/content".to_owned(), maestro_models::SchemaKind::String)]),
        },
        label: "write".into(),
        prompt_snippet: Some("Create or overwrite files".into()),
        prompt_guidelines: Some(vec![
            "Use write only for new files or complete rewrites.".into(),
        ]),
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: $ptr::new(move |_id, input, signal, _update, _context| {
            let absolute_path = match resolve_existing_path(&input.path, &cwd) {
                Ok(path) => path,
                Err(error) => return Box::pin(std::future::ready(Err(diagnostic(&error)))),
            };
            let dir = dirname(&absolute_path);
            let operations = Arc::clone(&operations);
            let key = absolute_path.clone();
            with_file_mutation_queue(&key, move || {
                write_file(operations, (absolute_path, dir), input, signal)
            })
        }),
        render_call: Some($ptr::new(render::render_call)),
        render_result: Some(crate::ToolRenderResult::Raw($ptr::new(render::render_result) as _)),
    }
}

/// Build the executable `write` tool with no caller context.
#[must_use]
pub fn create_write_tool(cwd: String, options: WriteToolOptions) -> AgentTool {
    wrap_tool_definition(create_write_tool_definition::<()>(cwd, options), None)
}

 };
}
factories!(Arc, [+ Send]);
/// Caller-local factories over the same file operations.
pub mod local {
    use super::{
        Arc, NativeWriteOperations, Tool, WriteToolInput, WriteToolOptions, diagnostic, dirname,
        json, render, resolve_existing_path, with_file_mutation_queue, write_file,
    };
    use crate::tool_definition_wrapper::local::wrap_tool_definition;
    use crate::types::local::ToolDefinition;
    use std::rc::Rc;
    /// Caller-local executable tool.
    type AgentTool = maestro_agent::AgentToolRecord<maestro_models::Local>;
    factories!(Rc, []);
}
