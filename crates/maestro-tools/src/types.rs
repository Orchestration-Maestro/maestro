//! Tool definitions: the executable declaration plus prompt and render capabilities.
use crate::renderer_state::ToolRendererState;
pub use crate::result_renderer::{ToolRenderResult, ToolRenderResultCallback};

use maestro_agent::{
    AgentToolResult, AgentToolUpdateCallback, PrepareArguments, ToolExecutionMode,
};
use maestro_models::{Cancellation, DiagnosticErrorInfo, Local, Threaded, Tool};
use maestro_theme::{LiveTheme, SyntaxHighlighter};
use maestro_tui::{TerminalImage, tui::ComponentHandle};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

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

/// Existing host effects supplied to tool renderers.
pub struct ToolRenderEnvironment {
    /// The caller's terminal runtime, which owns submitted preview futures.
    pub runtime: Rc<dyn maestro_tui::tui::TuiRuntime>,
    /// Absolute path of the documentation README, supplied by the application.
    pub readme_path: String,
    /// The published theme, read on every use by renderers, key hints and images.
    pub live_theme: LiveTheme,
    /// Classifies code for explicit highlighting.
    pub highlighter: Rc<dyn SyntaxHighlighter>,
    /// Image support of the terminal showing the row.
    pub terminal_image: TerminalImage,
}

/// Shared state a row hands to both of a tool's renderers.
pub struct ToolRenderContext<'a, S = Value> {
    /// Arguments as the model supplied them so far, borrowed for this call only.
    pub args: Option<&'a Value>,
    /// Stable id of the tool call.
    pub tool_call_id: String,
    /// Requests a redraw of the row; must not strongly retain its owning writer or runtime.
    pub invalidate: Rc<dyn Fn()>,
    /// Existing host effects used by rendering.
    pub environment: Rc<ToolRenderEnvironment>,
    /// Component this renderer returned for the row last time.
    pub last_component: Option<ComponentHandle>,
    /// State shared by both renderers across redraws.
    pub state: Rc<RefCell<S>>,
    /// Borrowed typed-state owner for native or custom callback delegation.
    pub renderer_state: Option<&'a ToolRendererState>,
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

impl<S> ToolRenderContext<'_, S> {
    /// Borrow the existing row owner when the host supplies one.
    #[must_use]
    pub fn renderer_state(&self) -> Option<&ToolRendererState> {
        self.renderer_state
    }
    /// Preserve the supplied context, selecting only another typed state handle.
    #[must_use]
    pub fn with_state<T>(&self, state: Rc<RefCell<T>>) -> ToolRenderContext<'_, T> {
        ToolRenderContext {
            args: self.args,
            tool_call_id: self.tool_call_id.clone(),
            invalidate: Rc::clone(&self.invalidate),
            environment: Rc::clone(&self.environment),
            last_component: self.last_component.clone(),
            state,
            renderer_state: self.renderer_state,
            cwd: self.cwd.clone(),
            phase: self.phase,
            expanded: self.expanded,
            show_images: self.show_images,
            is_error: self.is_error,
        }
    }
}

/// Draws a tool call from its borrowed arguments, or reports why it cannot.
pub type ToolRenderCall<S, E = Local> =
    <E as maestro_models::Execution>::Shared<<E as ToolExecution>::RenderCall<S>>;

/// Callable signatures for tool definitions in the agent's execution family.
pub trait ToolExecution: maestro_agent::AgentExecution {
    /// Definition execution with an owned per-call context.
    type Definition<C, P, D, I: maestro_agent::AgentExecution>: Fn(
            String,
            P,
            Option<Cancellation>,
            Option<AgentToolUpdateCallback<D, I>>,
            Option<C>,
        ) -> I::Future<'static, Result<AgentToolResult<D>, DiagnosticErrorInfo>>
        + ?Sized;
    /// The retained call renderer follows storage affinity, not invocation.
    ///
    /// The theme operand is resolved by the renderer itself, so a renderer that never
    /// reads it still draws when no theme is published.
    type RenderCall<S>: Fn(
            Option<&Value>,
            &LiveTheme,
            &ToolRenderContext<'_, S>,
        ) -> Result<ComponentHandle, DiagnosticErrorInfo>
        + ?Sized;
    /// The retained result renderer follows storage affinity.
    ///
    /// The theme operand is resolved by the renderer itself, so a renderer that never
    /// reads it still draws when no theme is published.
    type RenderResult<D, S>: Fn(
            &crate::output::render_utils::ToolRenderResultLike<'_, D>,
            ToolRenderResultOptions,
            &LiveTheme,
            &ToolRenderContext<'_, S>,
        ) -> Result<ComponentHandle, DiagnosticErrorInfo>
        + ?Sized;
    /// Project argument preparation into the local invocation, without executing it.
    fn local_prepare(prepare: PrepareArguments<Self>) -> PrepareArguments<Local>;
    /// Fresh context construction before each execution.
    type ContextFactory<C>: Fn() -> Result<C, DiagnosticErrorInfo> + ?Sized;
}
/// One callable declaration with concrete pointer and thread-safety bounds.
macro_rules! family {
    ($mode:ty, [$($bound:tt)*], $local:expr) => {
        impl ToolExecution for $mode {
            type Definition<C, P, D, I: maestro_agent::AgentExecution> = dyn Fn(String, P, Option<Cancellation>, Option<AgentToolUpdateCallback<D, I>>, Option<C>) -> I::Future<'static, Result<AgentToolResult<D>, DiagnosticErrorInfo>> $($bound)*;
            type RenderCall<S> = dyn Fn(Option<&Value>, &LiveTheme, &ToolRenderContext<'_, S>) -> Result<ComponentHandle, DiagnosticErrorInfo> $($bound)*;
            type RenderResult<D,S> = dyn Fn(&crate::output::render_utils::ToolRenderResultLike<'_, D>, ToolRenderResultOptions, &LiveTheme, &ToolRenderContext<'_, S>) -> Result<ComponentHandle, DiagnosticErrorInfo> $($bound)*;
            fn local_prepare(prepare: PrepareArguments<Self>) -> PrepareArguments<Local> { ($local)(prepare) }
            type ContextFactory<C> = dyn Fn() -> Result<C, DiagnosticErrorInfo> $($bound)*;
        }
    };
}
family!(Local, [], std::convert::identity);
#[cfg(not(target_arch = "wasm32"))]
family!(Threaded, [+ Send + Sync], |prepare: PrepareArguments<Threaded>| Rc::new(move |args: &mut Value| prepare(args)));
#[cfg(target_arch = "wasm32")]
family!(Threaded, [], std::convert::identity);

/// Executes a tool with its call id, arguments, signal, observer and context.
pub type ExecuteToolDefinition<C, P, D, E = Threaded, I = E> =
    <E as maestro_models::Execution>::Shared<<E as ToolExecution>::Definition<C, P, D, I>>;
/// Supplies a fresh caller context or its original failure.
pub type ToolContextFactory<C, E = Threaded> =
    <E as maestro_models::Execution>::Shared<<E as ToolExecution>::ContextFactory<C>>;

/// An executable tool with prompt text and optional renderers.
///
/// `C` is the caller context, `P` the parameters, `D` the result details and `S` the
/// renderers' shared row state.
pub struct ToolDefinitionRecord<
    C = (),
    P = Value,
    D = Value,
    S = Value,
    E: ToolExecution = Threaded,
    I: maestro_agent::AgentExecution = E,
> {
    /// Model-facing declaration.
    pub definition: Tool<E::SchemaCode>,
    /// Display label.
    pub label: String,
    /// One-line description for the system prompt.
    pub prompt_snippet: Option<String>,
    /// Guidelines appended to the system prompt.
    pub prompt_guidelines: Option<Vec<String>>,
    /// Frame selection for the renderers.
    pub render_shell: Option<ToolRenderShell>,
    /// Argument preparation; see [`PrepareArguments`] for the mutable input contract.
    pub prepare_arguments: Option<PrepareArguments<E>>,
    /// Scheduling preference.
    pub execution_mode: Option<ToolExecutionMode>,
    /// Execution callback.
    pub execute: ExecuteToolDefinition<C, P, D, E, I>,
    /// Call renderer.
    pub render_call: Option<ToolRenderCall<S, E>>,
    /// Result renderer.
    pub render_result: Option<ToolRenderResult<D, S, E>>,
}

/// One identity implementation for each concrete namespace.
macro_rules! definition_identity {
    ($mode:ty) => {
        /// Executable definition in this namespace's concrete callback family.
        pub type ToolDefinition<C = (), P = Value, D = Value, S = Value, I = $mode> =
            ToolDefinitionRecord<C, P, D, S, $mode, I>;
        /// Return the definition unchanged, fixing its types for the caller.
        #[must_use]
        pub fn define_tool<C, P, D, S, I: maestro_agent::AgentExecution>(
            tool: ToolDefinition<C, P, D, S, I>,
        ) -> ToolDefinition<C, P, D, S, I> {
            tool
        }
    };
}
definition_identity!(Threaded);
/// Definitions retaining caller-local callbacks and context factories.
pub mod local {
    use super::ToolDefinitionRecord;
    use maestro_models::Local;
    use serde_json::Value;
    /// Caller-local definition callback.
    pub type ExecuteToolDefinition<C, P, D> = super::ExecuteToolDefinition<C, P, D, Local>;
    /// Caller-local fresh context factory.
    pub type ToolContextFactory<C> = super::ToolContextFactory<C, Local>;
    definition_identity!(Local);
}
