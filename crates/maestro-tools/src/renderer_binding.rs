//! Renderer-only projections without retaining executable definitions.
use crate::{
    PreparedToolRenderResult, ToolDefinitionRecord, ToolExecution, ToolRenderCall,
    ToolRenderContext, ToolRenderResult, ToolRenderShell, ToolRendererState,
};
use maestro_models::DiagnosticErrorInfo;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::rc::Rc;

/// Factory retaining a callback while borrowing each operation's persisted input.
type PrepareResult = dyn for<'a> Fn(
    crate::output::render_utils::ToolRenderResultLike<'a, Value>,
) -> Result<PreparedToolRenderResult<'a>, DiagnosticErrorInfo>;

/// Erased result preparation without retaining an executable definition.
#[derive(Clone)]
pub struct ToolRenderResultBinding {
    /// One preparation owner for both typed and raw callbacks.
    prepare: Rc<PrepareResult>,
}
impl ToolRenderResultBinding {
    /// Prepare one result operation, borrowing raw inputs or owning decoded details.
    ///
    /// # Errors
    /// Returns the original conversion diagnostic.
    pub fn prepare<'a>(
        &self,
        result: crate::output::render_utils::ToolRenderResultLike<'a, Value>,
    ) -> Result<PreparedToolRenderResult<'a>, DiagnosticErrorInfo> {
        (self.prepare)(result)
    }
    /// Project native and custom prepared callbacks through the same row owner.
    fn project<D: 'static, S: Default + 'static, E: ToolExecution>(
        callback: ToolRenderResult<D, S, E>,
        decode: impl Fn(&Value) -> Result<D, DiagnosticErrorInfo> + 'static,
    ) -> Self {
        Self {
            prepare: Rc::new(move |result| {
                let prepared = callback.prepare_raw(result, &decode)?;
                Ok(PreparedToolRenderResult {
                    render: Box::new(move |options, theme, row| {
                        let owner = row.state.borrow().clone();
                        prepared.render(options, theme, &context::<S>(row, &owner))
                    }),
                })
            }),
        }
    }
}
/// Concrete local pointer required by coherence against core's identity conversion.
type LocalResultCallback = Rc<
    dyn Fn(
        &crate::output::render_utils::ToolRenderResultLike<'_, Value>,
        crate::ToolRenderResultOptions,
        &maestro_theme::LiveTheme,
        &ToolRenderContext<'_, ToolRendererState>,
    ) -> Result<maestro_tui::tui::ComponentHandle, DiagnosticErrorInfo>,
>;
impl From<LocalResultCallback> for ToolRenderResultBinding {
    fn from(callback: LocalResultCallback) -> Self {
        Self {
            prepare: Rc::new(move |result| {
                Ok(PreparedToolRenderResult::raw::<maestro_models::Local>(
                    Rc::clone(&callback),
                    result,
                ))
            }),
        }
    }
}

/// Cloned renderer handles over a row's typed state owner.
#[derive(Clone)]
pub struct ToolRendererBinding {
    /// Optional tool-owned frame policy.
    pub render_shell: Option<ToolRenderShell>,
    /// Borrowed arguments and erased row state.
    pub render_call: Option<ToolRenderCall<ToolRendererState>>,
    /// Persisted details and erased row state.
    pub render_result: Option<ToolRenderResultBinding>,
}
/// Select typed state, releasing the map borrow before returning to caller code.
fn context<'a, S: Default + 'static>(
    row: &'a ToolRenderContext<'_, ToolRendererState>,
    owner: &'a ToolRendererState,
) -> ToolRenderContext<'a, S> {
    let state = owner.state::<S>();
    let mut context = row.with_state(state);
    context.renderer_state = Some(owner);
    context
}
impl ToolRendererBinding {
    /// Clone custom renderers, decoding only details required by typed callbacks.
    #[must_use]
    pub fn from_definition<C, P, D, S, E: ToolExecution, I: maestro_agent::AgentExecution>(
        definition: &ToolDefinitionRecord<C, P, D, S, E, I>,
    ) -> Self
    where
        D: DeserializeOwned + 'static,
        S: Default + 'static,
    {
        let result = definition
            .render_result
            .clone()
            .map(|callback| ToolRenderResultBinding::project::<D, S, E>(callback, decode::<D>));
        Self::new(definition, result)
    }
    /// Clone factory raw callbacks without requiring a decoder for execution details.
    ///
    /// # Errors
    /// Rejects a typed result slot that requires the typed constructor.
    pub fn from_raw<
        C,
        P,
        D,
        S: Default + 'static,
        E: ToolExecution,
        I: maestro_agent::AgentExecution,
    >(
        definition: &ToolDefinitionRecord<C, P, D, S, E, I>,
    ) -> Result<Self, DiagnosticErrorInfo> {
        let result = match &definition.render_result {
            Some(ToolRenderResult::Typed(_)) => {
                return Err(DiagnosticErrorInfo::error(
                    "Typed result callback requires ToolRendererBinding::from_definition",
                ));
            }
            Some(ToolRenderResult::Raw(callback)) => {
                Some(ToolRenderResultBinding::project::<Value, S, E>(
                    ToolRenderResult::<Value, S, E>::Raw(callback.clone()),
                    decode::<Value>,
                ))
            }
            None => None,
        };
        Ok(Self::new(definition, result))
    }
    /// Share call projection between native and custom result representations.
    fn new<C, P, D, S: Default + 'static, E: ToolExecution, I: maestro_agent::AgentExecution>(
        definition: &ToolDefinitionRecord<C, P, D, S, E, I>,
        render_result: Option<ToolRenderResultBinding>,
    ) -> Self {
        Self {
            render_shell: definition.render_shell,
            render_call: definition.render_call.as_ref().map(|callback| {
                let callback = callback.clone();
                Rc::new(
                    move |args: Option<&Value>,
                          theme: &maestro_theme::LiveTheme,
                          row: &ToolRenderContext<'_, ToolRendererState>| {
                        let owner = row.state.borrow().clone();
                        callback(args, theme, &context::<S>(row, &owner))
                    },
                ) as ToolRenderCall<ToolRendererState>
            }),
            render_result,
        }
    }
}

/// Decode only the custom callback's present details at the diagnostic boundary.
fn decode<D: DeserializeOwned>(value: &Value) -> Result<D, DiagnosticErrorInfo> {
    D::deserialize(value).map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}
