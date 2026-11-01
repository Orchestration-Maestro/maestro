//! The `read` tool: text and image reads through replaceable operations.

mod execution;
mod native;
mod render;
mod text;

use crate::{
    output::truncate::{DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationResult},
    path_utils::resolve_read_path,
    tool_definition_wrapper::wrap_tool_definition,
    types::ToolDefinition,
    utils::diagnostics::diagnostic,
};
use execution::Reader;
use maestro_agent::AgentTool;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Model, Tool};
pub(crate) use native::{NativeReadOperations, access_file};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Arguments of one read.
#[derive(Clone, Debug, Deserialize)]
pub struct ReadToolInput {
    /// Path to the file, relative or absolute.
    pub path: String,
    /// One-based line to start from.
    pub offset: Option<f64>,
    /// Maximum number of lines to read.
    pub limit: Option<f64>,
}

/// Read observations with optional native truncation.
pub type ReadToolDetails = maestro_tool::tools::read::ReadToolDetails<Option<TruncationResult>>;

/// The file access a read performs; replace it to read through another system.
pub trait ReadOperations: Send + Sync {
    /// Read the file's bytes.
    fn read_file(&self, absolute_path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>>;

    /// Fail unless the file is readable.
    fn access(&self, absolute_path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;

    /// Return the file's image MIME type, or `None` for anything read as text.
    fn detect_image_mime_type(
        &self,
        _absolute_path: String,
    ) -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(None)))
    }
}

/// Options of the read tool.
#[derive(Default)]
pub struct ReadToolOptions {
    /// Whether images are fitted to the inline limits; `None` means yes.
    pub auto_resize_images: Option<bool>,
    /// File access to use; `None` reads the local filesystem.
    pub operations: Option<Arc<dyn ReadOperations>>,
}

/// A caller context that knows the active model.
pub trait ReadContext {
    /// The model in use right now, if known.
    fn model(&self) -> Option<Arc<Model>>;
}

impl ReadContext for () {
    fn model(&self) -> Option<Arc<Model>> {
        None
    }
}

/// One factory body with concrete callback ownership.
macro_rules! factories {
 ($ptr:ident, [$($bound:tt)*]) => {
/// Build the `read` tool definition.
///
/// The path is resolved against `cwd` when the tool is called, before any
/// asynchronous work, so the returned future is unaffected by later working
/// directory changes.
#[must_use]
pub fn create_read_tool_definition<C: ReadContext + 'static $($bound)*>(
    cwd: String,
    options: ReadToolOptions,
) -> ToolDefinition<C, ReadToolInput, ReadToolDetails> {
    let auto_resize = options.auto_resize_images.unwrap_or(true);
    let reader = Arc::new(Reader {
        operations: options
            .operations
            .unwrap_or_else(|| Arc::new(NativeReadOperations)),
        auto_resize,
    });
    let description = format!(
        "Read the contents of a file. Supports text files and images (jpg, png, gif, webp). Images are sent as attachments. For text files, output is truncated to {DEFAULT_MAX_LINES} lines or {}KB (whichever is hit first). Use offset/limit for large files. When you need the full file, continue with offset until complete.",
        DEFAULT_MAX_BYTES / 1024
    );
    ToolDefinition {
        definition: Tool {
            name: "read".into(),
            description,
            parameters: maestro_models::ParameterSchema::authored(json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {"type": "string", "description": "Path to the file to read (relative or absolute)"},
                    "offset": {"type": "number", "description": "Line number to start reading from (1-indexed)"},
                    "limit": {"type": "number", "description": "Maximum number of lines to read"},
                },
            }), [("".to_owned(), maestro_models::SchemaKind::Object), ("/properties/path".to_owned(), maestro_models::SchemaKind::String), ("/properties/offset".to_owned(), maestro_models::SchemaKind::Number), ("/properties/limit".to_owned(), maestro_models::SchemaKind::Number)]),
        },
        label: "read".into(),
        prompt_snippet: Some("Read file contents".into()),
        prompt_guidelines: Some(vec![
            "Use read to examine files instead of cat or sed.".into(),
        ]),
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: $ptr::new(move |_id, input, signal, _update, context| {
            match resolve_read_path(&input.path, &cwd) {
                Ok(path) => {
                    let reader = Arc::clone(&reader);
                    Box::pin(async move { reader.read(path, input, signal, context).await })
                }
                Err(error) => Box::pin(std::future::ready(Err(diagnostic(&error)))),
            }
        }),
        render_call: Some($ptr::new(render::render_call)),
        render_result: Some(crate::ToolRenderResult::Raw($ptr::new(render::render_result) as _)),
    }
}

/// Build the executable `read` tool with no caller context.
#[must_use]
pub fn create_read_tool(cwd: String, options: ReadToolOptions) -> AgentTool {
    wrap_tool_definition(create_read_tool_definition::<()>(cwd, options), None)
}

 };
}
factories!(Arc, [+ Send]);
/// Caller-local factories over the same file operations.
pub mod local {
    use super::{
        Arc, DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, NativeReadOperations, ReadContext,
        ReadToolDetails, ReadToolInput, ReadToolOptions, Reader, Tool, diagnostic, json, render,
        resolve_read_path,
    };
    use crate::tool_definition_wrapper::local::wrap_tool_definition;
    use crate::types::local::ToolDefinition;
    use std::rc::Rc;
    /// Caller-local executable tool.
    type AgentTool = maestro_agent::AgentToolRecord<maestro_models::Local>;
    factories!(Rc, []);
}
