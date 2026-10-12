//! The `read` tool: text and image reads through replaceable operations.

mod execution;
mod native;
mod text;

use crate::{
    output::truncate::{DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationResult},
    path_utils::resolve_read_path,
    tool_definition_wrapper::wrap_tool_definition,
    types::ToolDefinition,
};
use execution::Reader;
use maestro_agent::AgentTool;
use maestro_models::{
    BoxFuture, DiagnosticErrorInfo, DiagnosticInput, Model, Tool, extract_diagnostic_error,
};
use native::NativeReadOperations;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// A failure carrying only its message.
fn failure(message: &str) -> DiagnosticErrorInfo {
    extract_diagnostic_error(DiagnosticInput::Text(message))
}

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

/// What a read reports beyond its content.
#[derive(Serialize)]
pub struct ReadToolDetails {
    /// The automatic truncation applied to text, when there was one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<TruncationResult>,
}

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

/// Build the `read` tool definition.
///
/// The path is resolved against `cwd` when the tool is called, before any
/// asynchronous work, so the returned future is unaffected by later working
/// directory changes.
#[must_use]
pub fn create_read_tool_definition<C: ReadContext + Send + 'static>(
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
            parameters: json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": {"type": "string", "description": "Path to the file to read (relative or absolute)"},
                    "offset": {"type": "number", "description": "Line number to start reading from (1-indexed)"},
                    "limit": {"type": "number", "description": "Maximum number of lines to read"},
                },
            }),
        },
        label: "read".into(),
        prompt_snippet: Some("Read file contents".into()),
        prompt_guidelines: Some(vec![
            "Use read to examine files instead of cat or sed.".into(),
        ]),
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_id, input, signal, _update, context| {
            match resolve_read_path(&input.path, &cwd) {
                Ok(path) => {
                    let reader = Arc::clone(&reader);
                    Box::pin(async move { reader.read(path, input, signal, context).await })
                }
                Err(error) => Box::pin(std::future::ready(Err(native::diagnostic(&error)))),
            }
        }),
        render_call: None,
        render_result: None,
    }
}

/// Build the executable `read` tool with no caller context.
#[must_use]
pub fn create_read_tool(cwd: String, options: ReadToolOptions) -> AgentTool {
    wrap_tool_definition(create_read_tool_definition::<()>(cwd, options), None)
}
