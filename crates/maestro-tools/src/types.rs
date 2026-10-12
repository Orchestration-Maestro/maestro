//! Tool definitions: the executable declaration plus prompt and render capabilities.

use maestro_agent::{
    AgentToolResult, AgentToolUpdateCallback, PrepareArguments, ToolExecutionMode,
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, Tool};
use maestro_theme::Theme;
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// How a tool's calls and results are framed on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRenderShell {
    /// The host frames the tool with its default container.
    Default,
    /// The tool's renderers draw their own frame.
    SelfRendered,
}

/// Flags that change how a result is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRenderResultOptions {
    /// Whether the result is expanded.
    pub expanded: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// Execution progress flags of a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRenderPhase {
    /// Whether execution has started.
    pub execution_started: bool,
    /// Whether the arguments are complete.
    pub args_complete: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// Shared state a row hands to both of a tool's renderers.
pub struct ToolRenderContext<S = Value, P = Value> {
    /// Arguments as the model supplied them so far.
    pub args: Rc<RefCell<P>>,
    /// Stable id of the tool call.
    pub tool_call_id: String,
    /// Requests a redraw of the row.
    pub invalidate: Rc<dyn Fn()>,
    /// Component this renderer returned for the row last time.
    pub last_component: Option<ComponentHandle>,
    /// State shared by both renderers across redraws.
    pub state: Rc<RefCell<S>>,
    /// Working directory of the session.
    pub cwd: String,
    /// Execution progress of the row.
    pub phase: ToolRenderPhase,
    /// Whether the row is expanded.
    pub expanded: bool,
    /// Whether images are shown.
    pub show_images: bool,
    /// Whether the result is an error.
    pub is_error: bool,
}

/// Draws a tool call from its arguments.
pub type ToolRenderCall<P, S> =
    Rc<dyn Fn(Rc<RefCell<P>>, &Theme, &ToolRenderContext<S, P>) -> ComponentHandle>;

/// Draws a tool result.
pub type ToolRenderResult<P, D, S> = Rc<
    dyn Fn(
        &AgentToolResult<D>,
        ToolRenderResultOptions,
        &Theme,
        &ToolRenderContext<S, P>,
    ) -> ComponentHandle,
>;

/// Executes a tool with its call id, arguments, signal, progress observer and caller context.
#[cfg(not(target_arch = "wasm32"))]
pub type ExecuteToolDefinition<C, P, D> = Arc<
    dyn Fn(
            String,
            P,
            Option<Cancellation>,
            Option<AgentToolUpdateCallback<D>>,
            Option<C>,
        ) -> BoxFuture<Result<AgentToolResult<D>, DiagnosticErrorInfo>>
        + Send
        + Sync,
>;

/// Executes a browser tool with its call id, arguments, signal, progress observer and caller context.
#[cfg(target_arch = "wasm32")]
pub type ExecuteToolDefinition<C, P, D> = Arc<
    dyn Fn(
        String,
        P,
        Option<Cancellation>,
        Option<AgentToolUpdateCallback<D>>,
        Option<C>,
    ) -> BoxFuture<Result<AgentToolResult<D>, DiagnosticErrorInfo>>,
>;

/// Supplies a fresh caller context for each execution, or the reason there is none.
#[cfg(not(target_arch = "wasm32"))]
pub type ToolContextFactory<C> = Arc<dyn Fn() -> Result<C, DiagnosticErrorInfo> + Send + Sync>;

/// Supplies a fresh browser caller context for each execution, or the reason there is none.
#[cfg(target_arch = "wasm32")]
pub type ToolContextFactory<C> = Arc<dyn Fn() -> Result<C, DiagnosticErrorInfo>>;

/// An executable tool with prompt text and optional renderers.
///
/// `C` is the caller context, `P` the parameters, `D` the result details and `S` the
/// renderers' shared row state.
pub struct ToolDefinition<C = (), P = Value, D = Value, S = Value> {
    /// Model-facing declaration.
    pub definition: Tool,
    /// Display label.
    pub label: String,
    /// One-line description for the system prompt.
    pub prompt_snippet: Option<String>,
    /// Guidelines appended to the system prompt.
    pub prompt_guidelines: Option<Vec<String>>,
    /// Frame selection for the renderers.
    pub render_shell: Option<ToolRenderShell>,
    /// Argument preparation run before schema validation.
    pub prepare_arguments: Option<PrepareArguments>,
    /// Scheduling preference.
    pub execution_mode: Option<ToolExecutionMode>,
    /// Execution callback.
    pub execute: ExecuteToolDefinition<C, P, D>,
    /// Call renderer.
    pub render_call: Option<ToolRenderCall<P, S>>,
    /// Result renderer.
    pub render_result: Option<ToolRenderResult<P, D, S>>,
}

/// Return the definition unchanged, fixing its type parameters for the caller.
#[must_use]
pub fn define_tool<C, P, D, S>(tool: ToolDefinition<C, P, D, S>) -> ToolDefinition<C, P, D, S> {
    tool
}
