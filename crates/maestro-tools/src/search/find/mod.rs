//! File finding through supplied operations or the native search program.
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
use std::{rc::Rc, sync::Arc};
/// Arguments of a file search.
#[derive(Deserialize)]
pub struct FindToolInput {
    /// Glob pattern supplied to the search operation.
    pub pattern: String,
    /// Search directory; absent or empty means the working directory.
    pub path: Option<String>,
    /// Requested result limit; absent means 1000.
    pub limit: Option<f64>,
}
/// Limits reported by a file search.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FindToolDetails {
    /// Byte selection, when the body was cut.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<TruncationResult>,
    /// Result limit reported by the search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_limit_reached: Option<f64>,
}
/// Options forwarded to supplied glob operations.
pub struct FindGlobOptions {
    /// Ignore patterns, in their supplied order.
    pub ignore: Vec<String>,
    /// Original effective result limit.
    pub limit: f64,
}
/// Replaceable existence and glob effects.
pub trait FindOperations: Send + Sync {
    /// Whether the search path exists.
    fn exists(&self, absolute_path: String) -> BoxFuture<Result<bool, DiagnosticErrorInfo>>;
    /// Ordered matching paths, relative to `cwd` or absolute.
    fn glob(
        &self,
        pattern: String,
        cwd: String,
        options: FindGlobOptions,
    ) -> BoxFuture<Result<Vec<String>, DiagnosticErrorInfo>>;
}
/// File search configuration.
#[derive(Default)]
pub struct FindToolOptions {
    /// Replacement operations; absent uses the native search program.
    pub operations: Option<Arc<dyn FindOperations>>,
}
/// Build a file search definition without resolving paths or acquiring programs.
#[must_use]
pub fn create_find_tool_definition<C: Send + 'static>(
    cwd: String,
    options: FindToolOptions,
    tools_manager: Arc<ToolsManager>,
) -> ToolDefinition<C, FindToolInput, FindToolDetails> {
    let finder = Arc::new(execute::Finder {
        cwd,
        operations: options.operations,
        tools_manager,
    });
    ToolDefinition {
        definition: Tool { name: "find".into(), description: "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to 1000 results or 50KB (whichever is hit first).".into(), parameters: json!({"type":"object", "required":["pattern"], "properties": {
            "pattern": {"type":"string", "description":"Glob pattern to match files, e.g. '*.ts', '**/*.json', or 'src/**/*.spec.ts'"},
            "path": {"type":"string", "description":"Directory to search in (default: current directory)"},
            "limit": {"type":"number", "description":"Maximum number of results (default: 1000)"}
        }}) }, label: "find".into(), prompt_snippet: Some("Find files by glob pattern (respects .gitignore)".into()), prompt_guidelines: None, render_shell: None, prepare_arguments: None, execution_mode: None,
        execute: Arc::new(move |_id, input, signal, _update, _context| {
            let finder = Arc::clone(&finder);
            Box::pin(async move { finder.find(input, signal).await })
        }), render_call: Some(Rc::new(render::render_call)), render_result: Some(crate::ToolRenderResult::Raw(Rc::new(render::render_result))),
    }
}
/// Build the executable file search with no caller context.
#[must_use]
pub fn create_find_tool(
    cwd: String,
    options: FindToolOptions,
    tools_manager: Arc<ToolsManager>,
) -> AgentTool {
    wrap_tool_definition(
        create_find_tool_definition::<()>(cwd, options, tools_manager),
        None,
    )
}
