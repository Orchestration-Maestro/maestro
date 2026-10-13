//! Prepared original-snapshot replacements through replaceable file operations.

mod execute;
mod prepare;

use crate::{
    file_mutation_queue::with_file_mutation_queue,
    path_utils::resolve_to_cwd,
    tool_definition_wrapper::wrap_tool_definition,
    types::{ToolDefinition, ToolRenderShell},
    utils::diagnostics::{diagnostic, failure},
};
use execute::{NativeEditOperations, edit_file};
use maestro_agent::AgentTool;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Tool};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

/// Arguments of one original-snapshot edit.
#[derive(Clone, Debug, Deserialize)]
pub struct EditToolInput {
    /// Authored relative or absolute path.
    pub path: String,
    /// Ordered replacements matched against the original file.
    pub edits: Vec<crate::edit_diff::Edit>,
}

/// Numbered diff returned after a successful write.
pub type EditToolDetails = crate::edit_diff::EditDiffResult;

/// File operations used by an edit.
pub trait EditOperations: Send + Sync {
    /// Read the target's raw bytes.
    fn read_file(&self, absolute_path: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>>;
    /// Write UTF-8 content to the target.
    fn write_file(
        &self,
        absolute_path: String,
        content: String,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;
    /// Check read and write access to the target.
    fn access(&self, absolute_path: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>>;
}

/// Operations selected for an edit tool.
#[derive(Default)]
pub struct EditToolOptions {
    /// Supplied operations; `None` selects the local filesystem.
    pub operations: Option<Arc<dyn EditOperations>>,
}

/// Build an edit definition with argument preparation and no render callbacks.
///
/// Empty lists fail before path resolution. Successful resolution enters the
/// [file mutation queue](crate::file_mutation_queue) before the future is polled.
#[must_use]
pub fn create_edit_tool_definition<C: 'static>(
    cwd: String,
    options: EditToolOptions,
) -> ToolDefinition<C, EditToolInput, EditToolDetails> {
    let operations = options
        .operations
        .unwrap_or_else(|| Arc::new(NativeEditOperations));
    ToolDefinition {
        definition: Tool {
            name: "edit".into(),
            description: "Edit a single file using exact text replacement. Every edits[].oldText must match a unique, non-overlapping region of the original file. If two changes affect the same block or nearby lines, merge them into one edit instead of emitting overlapping edits. Do not include large unchanged regions just to connect distant changes.".into(),
            parameters: json!({
                "type": "object", "required": ["path", "edits"],
                "properties": {
                    "path": {"type": "string", "description": "Path to the file to edit (relative or absolute)"},
                    "edits": {"type": "array", "items": {
                        "type": "object", "required": ["oldText", "newText"],
                        "properties": {
                            "oldText": {"type": "string", "description": "Exact text for one targeted replacement. It must be unique in the original file and must not overlap with any other edits[].oldText in the same call."},
                            "newText": {"type": "string", "description": "Replacement text for this targeted edit."}
                        }, "additionalProperties": false
                    }, "description": "One or more targeted replacements. Each edit is matched against the original file, not incrementally. Do not include overlapping or nested edits. If two changes touch the same block or nearby lines, merge them into one edit instead."}
                }, "additionalProperties": false
            }),
        },
        label: "edit".into(),
        prompt_snippet: Some("Make precise file edits with exact text replacement, including multiple disjoint edits in one call".into()),
        prompt_guidelines: Some(vec![
            "Use edit for precise changes (edits[].oldText must match exactly)".into(),
            "When changing multiple separate locations in one file, use one edit call with multiple entries in edits[] instead of multiple edit calls".into(),
            "Each edits[].oldText is matched against the original file, not after earlier edits are applied. Do not emit overlapping or nested edits. Merge nearby changes into one edit.".into(),
            "Keep edits[].oldText as small as possible while still being unique in the file. Do not pad with large unchanged regions.".into(),
        ]),
        render_shell: Some(ToolRenderShell::SelfRendered),
        prepare_arguments: Some(Arc::new(|args| Ok(prepare::prepare_edit_arguments(args)))),
        execution_mode: None,
        execute: Arc::new(move |_id, input, signal, _update, _context| {
            if input.edits.is_empty() {
                return Box::pin(std::future::ready(Err(failure("edits must contain at least one replacement"))));
            }
            let absolute = match resolve_to_cwd(&input.path, &cwd) {
                Ok(path) => path,
                Err(error) => return Box::pin(std::future::ready(Err(diagnostic(&error)))),
            };
            let operations = Arc::clone(&operations);
            let key = absolute.clone();
            with_file_mutation_queue(&key, move || edit_file(operations, absolute, input, signal))
        }),
        render_call: None,
        render_result: None,
    }
}

/// Build the executable edit tool with no caller context.
#[must_use]
pub fn create_edit_tool(cwd: String, options: EditToolOptions) -> AgentTool {
    wrap_tool_definition(create_edit_tool_definition::<()>(cwd, options), None)
}
