//! Renderer-only projections without retaining executable definitions.
use crate::{
    ToolDefinition, ToolRenderCall, ToolRenderContext, ToolRenderResult, ToolRenderResultCallback,
    ToolRenderShell, ToolRendererState,
};
use maestro_models::DiagnosticErrorInfo;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::rc::Rc;

/// Cloned renderer handles over a row's typed state owner.
pub struct ToolRendererBinding {
    /// Optional tool-owned frame policy.
    pub render_shell: Option<ToolRenderShell>,
    /// Borrowed arguments and erased row state.
    pub render_call: Option<ToolRenderCall<ToolRendererState>>,
    /// Persisted details and erased row state.
    pub render_result: Option<ToolRenderResultCallback<Value, ToolRendererState>>,
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
    pub fn from_definition<C, P, D, S>(definition: &ToolDefinition<C, P, D, S>) -> Self
    where
        D: DeserializeOwned + 'static,
        S: Default + 'static,
    {
        let result = definition.render_result.clone().map(|callback| {
            Rc::new(
                move |result: &crate::output::render_utils::ToolRenderResultLike<'_, Value>,
                      options,
                      theme: &maestro_theme::Theme,
                      row: &ToolRenderContext<'_, ToolRendererState>| {
                    let owner = row.state.borrow().clone();
                    callback.invoke_raw(
                        (result, options),
                        theme,
                        &context::<S>(row, &owner),
                        decode::<D>,
                    )
                },
            ) as ToolRenderResultCallback<Value, ToolRendererState>
        });
        Self::new(definition, result)
    }
    /// Clone factory raw callbacks without requiring a decoder for execution details.
    pub(crate) fn from_raw<C, P, D, S: Default + 'static>(
        definition: &ToolDefinition<C, P, D, S>,
    ) -> Result<Self, DiagnosticErrorInfo> {
        let result = match &definition.render_result {
            Some(ToolRenderResult::Typed(_)) => {
                return Err(DiagnosticErrorInfo::error(
                    "Typed result callback requires ToolRendererBinding::from_definition",
                ));
            }
            Some(ToolRenderResult::Raw(callback)) => {
                let callback = Rc::clone(callback);
                Some(Rc::new(
                    move |result: &crate::output::render_utils::ToolRenderResultLike<'_, Value>,
                          options,
                          theme: &maestro_theme::Theme,
                          row: &ToolRenderContext<'_, ToolRendererState>| {
                        let owner = row.state.borrow().clone();
                        callback(result, options, theme, &context::<S>(row, &owner))
                    },
                )
                    as ToolRenderResultCallback<Value, ToolRendererState>)
            }
            None => None,
        };
        Ok(Self::new(definition, result))
    }
    /// Share call projection between native and custom result representations.
    fn new<C, P, D, S: Default + 'static>(
        definition: &ToolDefinition<C, P, D, S>,
        render_result: Option<ToolRenderResultCallback<Value, ToolRendererState>>,
    ) -> Self {
        Self {
            render_shell: definition.render_shell,
            render_call: definition.render_call.as_ref().map(|callback| {
                let callback = Rc::clone(callback);
                Rc::new(
                    move |args: Option<&Value>,
                          theme: &maestro_theme::Theme,
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
