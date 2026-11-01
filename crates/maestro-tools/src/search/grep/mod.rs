//! Content search through a native search program and replaceable file effects.
mod execute;
mod native;
mod render;
use crate::{
    output::truncate::TruncationResult, search::tools_manager::ToolsManager,
    tool_definition_wrapper::wrap_tool_definition, types::ToolDefinition,
};
use maestro_agent::AgentTool;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Tool};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
/// Arguments of a content search.
#[derive(Deserialize)]
pub struct GrepToolInput {
    /// Regular expression or literal search text.
    pub pattern: String,
    /// Search root; absent or empty uses the working directory.
    pub path: Option<String>,
    /// Optional native file filter.
    pub glob: Option<String>,
    /// Whether case is ignored.
    #[serde(rename = "ignoreCase")]
    pub ignore_case: Option<bool>,
    /// Whether the pattern is literal.
    pub literal: Option<bool>,
    /// Lines surrounding each match.
    pub context: Option<f64>,
    /// Maximum match events, defaulting to 100.
    pub limit: Option<f64>,
}
/// Active limits of a content search.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrepToolDetails {
    /// Byte selection, if the body was cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<TruncationResult>,
    /// Effective match limit, if reached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_limit_reached: Option<f64>,
    /// Whether any displayed source line was clipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_truncated: Option<bool>,
}
/// Replaceable filesystem inspection and context reads; search remains native.
pub trait GrepOperations: Send + Sync {
    /// Inspect the resolved root, failing if it does not exist.
    fn is_directory(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>>;
    /// Read context text from the named file.
    fn read_file(&self, absolute_path: String) -> BoxFuture<Result<String, DiagnosticErrorInfo>>;
}
/// Content search configuration.
#[derive(Default)]
pub struct GrepToolOptions {
    /// Replacement file effects; absent selects local files.
    pub operations: Option<Arc<dyn GrepOperations>>,
}
/// One factory body with concrete callback ownership.
macro_rules! factories {
 ($ptr:ident, [$($bound:tt)*]) => {
/// Build a content search without filesystem or acquisition effects.
#[must_use]
pub fn create_grep_tool_definition<C: 'static $($bound)*>(
    cwd: String,
    options: GrepToolOptions,
    tools_manager: Arc<ToolsManager>,
) -> ToolDefinition<C, GrepToolInput, GrepToolDetails> {
    let searcher = Arc::new(execute::Searcher {
        cwd,
        operations: options.operations,
        tools_manager,
    });
    ToolDefinition {
        definition: Tool { name: "grep".into(), description: "Search file contents for a pattern. Returns matching lines with file paths and line numbers. Respects .gitignore. Output is truncated to 100 matches or 50KB (whichever is hit first). Long lines are truncated to 500 chars.".into(), parameters: maestro_models::ParameterSchema::authored(json!({"type":"object","required":["pattern"],"properties": {
            "pattern":{"type":"string","description":"Search pattern (regex or literal string)"},
            "path":{"type":"string","description":"Directory or file to search (default: current directory)"},
            "glob":{"type":"string","description":"Filter files by glob pattern, e.g. '*.ts' or '**/*.spec.ts'"},
            "ignoreCase":{"type":"boolean","description":"Case-insensitive search (default: false)"},
            "literal":{"type":"boolean","description":"Treat pattern as literal string instead of regex (default: false)"},
            "context":{"type":"number","description":"Number of lines to show before and after each match (default: 0)"},
            "limit":{"type":"number","description":"Maximum number of matches to return (default: 100)"}
        }}), [("".to_owned(), maestro_models::SchemaKind::Object), ("/properties/pattern".to_owned(), maestro_models::SchemaKind::String), ("/properties/path".to_owned(), maestro_models::SchemaKind::String), ("/properties/glob".to_owned(), maestro_models::SchemaKind::String), ("/properties/ignoreCase".to_owned(), maestro_models::SchemaKind::Boolean), ("/properties/literal".to_owned(), maestro_models::SchemaKind::Boolean), ("/properties/context".to_owned(), maestro_models::SchemaKind::Number), ("/properties/limit".to_owned(), maestro_models::SchemaKind::Number)]) }, label: "grep".into(), prompt_snippet: Some("Search file contents for patterns (respects .gitignore)".into()), prompt_guidelines: None, render_shell: None, prepare_arguments: None, execution_mode: None,
        execute: $ptr::new(move |_id, input, signal, _update, _context| {
            let searcher = Arc::clone(&searcher);
            Box::pin(async move { searcher.search(input, signal).await })
        }), render_call: Some($ptr::new(render::render_call)), render_result: Some(crate::ToolRenderResult::Raw($ptr::new(render::render_result) as _)),
    }
}
/// Build the executable content search with no caller context.
#[must_use]
pub fn create_grep_tool(
    cwd: String,
    options: GrepToolOptions,
    tools_manager: Arc<ToolsManager>,
) -> AgentTool {
    wrap_tool_definition(
        create_grep_tool_definition::<()>(cwd, options, tools_manager),
        None,
    )
}

 };
}
factories!(Arc, [+ Send]);
/// Caller-local factories over the same file operations.
pub mod local {
    use super::{
        Arc, GrepToolDetails, GrepToolInput, GrepToolOptions, Tool, ToolsManager, execute, json,
        render,
    };
    use crate::tool_definition_wrapper::local::wrap_tool_definition;
    use crate::types::local::ToolDefinition;
    use std::rc::Rc;
    /// Caller-local executable tool.
    type AgentTool = maestro_agent::AgentToolRecord<maestro_models::Local>;
    factories!(Rc, []);
}
