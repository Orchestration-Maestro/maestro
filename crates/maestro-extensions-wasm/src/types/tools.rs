//! Authored tool definitions and registration metadata.
use crate::Presence;
use crate::{
    AbortSignal, AgentToolResult, AgentToolUpdateCallback, ExtensionContext, ExtensionFuture,
    ExtensionResult, ToolExecutionMode,
};
use maestro_request::parameter_schema::ParameterSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// Authored shell rendering choice.
pub enum RenderShell {
    /// Host-provided shell rendering.
    Default,
    #[serde(rename = "self")]
    /// Tool-provided shell rendering.
    SelfRendered,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Authored registration metadata.
pub struct ToolMetadata {
    /// Name.
    pub name: String,
    /// Label.
    pub label: String,
    /// Description.
    pub description: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Prompt snippet.
    pub prompt_snippet: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Prompt guidelines.
    pub prompt_guidelines: Presence<Vec<String>>,
    /// Parameters.
    pub parameters: ParameterSchema<crate::schema_code::SchemaCode>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Render shell.
    pub render_shell: Presence<RenderShell>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Execution mode.
    pub execution_mode: Presence<ToolExecutionMode>,
}
/// Synchronously prepares invocation arguments while retaining in-place changes in history.
///
/// The returned value is validated and executed independently of the retained arguments.
pub type PrepareArguments =
    Rc<dyn for<'a> Fn(&mut Value, crate::CallbackEmitter<'a>) -> ExtensionResult<Value>>;
/// Asynchronous execution receiving owned context, signal and update handles.
pub type ToolExecute = Rc<
    dyn Fn(
        String,
        Value,
        Option<AbortSignal>,
        Option<AgentToolUpdateCallback>,
        ExtensionContext,
    ) -> ExtensionFuture<'static, AgentToolResult>,
>;
/// A registered tool with optional preparation, asynchronous execution and optional
/// renderers.
pub struct ToolDefinition {
    /// Metadata.
    pub metadata: ToolMetadata,
    /// Prepare arguments.
    pub prepare_arguments: Option<PrepareArguments>,
    /// Execute.
    pub execute: ToolExecute,
    /// Renders the tool call row.
    pub render_call: Option<ToolRenderCall>,
    /// Renders the tool result row.
    pub render_result: Option<ToolRenderResult>,
}

/// Options of a result rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolRenderResultOptions {
    /// Whether the result view is expanded.
    pub expanded: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// State and previous components one tool row keeps across renderings.
#[derive(Default)]
pub(crate) struct ToolRow {
    /// One handle per concrete state type.
    states: RefCell<HashMap<TypeId, Rc<dyn Any>>>,
    /// The component each renderer slot returned last: call, then result.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) last: RefCell<[Option<crate::Component>; 2]>,
}

/// Execution progress of a tool row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolRenderPhase {
    /// Whether execution has started.
    pub execution_started: bool,
    /// Whether the arguments are complete.
    pub args_complete: bool,
    /// Whether the result is still streaming.
    pub is_partial: bool,
}

/// What a tool renderer reads about its row at one rendering.
#[derive(Clone)]
pub struct ToolRenderContext {
    /// Arguments as the model supplied them so far.
    pub args: Option<Value>,
    /// Stable id of the tool call.
    pub tool_call_id: String,
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
    /// The component this renderer returned for the row last time.
    pub last_component: Option<crate::Component>,
    /// Redraws this row.
    pub invalidate: Rc<dyn Fn() -> ExtensionResult<()>>,
    /// Opaque original native row, lent only by a native rendering invocation.
    pub native_row: Option<crate::NativeRow>,
    /// The row's state and previous components.
    pub(crate) row: Rc<ToolRow>,
}

impl ToolRenderContext {
    /// The row's state of type `S`, shared by both renderers of the row across renderings and
    /// created with its default on first use.
    #[must_use]
    pub fn state<S: Default + 'static>(&self) -> Rc<RefCell<S>> {
        let existing = self.row.states.borrow().get(&TypeId::of::<S>()).cloned();
        if let Some(state) = existing.and_then(|state| state.downcast().ok()) {
            return state;
        }
        let state = Rc::new(RefCell::new(S::default()));
        self.row
            .states
            .borrow_mut()
            .insert(TypeId::of::<S>(), state.clone() as Rc<dyn Any>);
        state
    }
}

/// Renders a tool call row from the arguments so far, synchronously.
pub type ToolRenderCall = Rc<
    dyn for<'a> Fn(
        Option<Value>,
        crate::Theme,
        &ToolRenderContext,
        crate::CallbackEmitter<'a>,
    ) -> ExtensionResult<crate::Component>,
>;

/// Owned rendering-result observation; execution results keep their existing record.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRenderResultLike {
    /// Original content, details and termination fields with their existing omission rules.
    #[serde(flatten)]
    pub result: AgentToolResult,
    /// Independent error observation supplied by the rendering caller.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub is_error: Presence<bool>,
}

/// Renders a tool result row, synchronously.
pub type ToolRenderResult = Rc<
    dyn for<'a> Fn(
        ToolRenderResultLike,
        ToolRenderResultOptions,
        crate::Theme,
        &ToolRenderContext,
        crate::CallbackEmitter<'a>,
    ) -> ExtensionResult<crate::Component>,
>;
