//! Result callback dispatch at typed execution and persisted JSON boundaries.
use crate::{
    ToolRenderContext, ToolRenderResultOptions, output::render_utils::ToolRenderResultLike,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::Theme;
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;
use std::rc::Rc;

/// Draws a borrowed result with its selected details representation.
pub type ToolRenderResultCallback<D, S> = Rc<
    dyn Fn(
        &ToolRenderResultLike<'_, D>,
        ToolRenderResultOptions,
        &Theme,
        &ToolRenderContext<'_, S>,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo>,
>;

/// One result slot, retaining custom typed callbacks or borrowed JSON selection.
pub enum ToolRenderResult<D, S> {
    /// A caller-defined details contract.
    Typed(ToolRenderResultCallback<D, S>),
    /// A renderer that selects only the JSON fields it consumes.
    Raw(ToolRenderResultCallback<Value, S>),
}
impl<D, S> Clone for ToolRenderResult<D, S> {
    fn clone(&self) -> Self {
        match self {
            Self::Typed(callback) => Self::Typed(Rc::clone(callback)),
            Self::Raw(callback) => Self::Raw(Rc::clone(callback)),
        }
    }
}
impl<D, S> ToolRenderResult<D, S> {
    /// Render native details, serializing only when a raw callback needs present details.
    ///
    /// # Errors
    /// Returns the original conversion or callback diagnostic.
    pub fn invoke_native<F>(
        &self,
        input: (&ToolRenderResultLike<'_, D>, ToolRenderResultOptions),
        theme: &Theme,
        context: &ToolRenderContext<'_, S>,
        serialize: F,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo>
    where
        F: FnOnce(&D) -> Result<Value, DiagnosticErrorInfo>,
    {
        let (result, options) = input;
        match self {
            Self::Typed(callback) => callback(result, options, theme, context),
            Self::Raw(callback) => {
                let details = result.details.map(serialize).transpose()?;
                callback(
                    &ToolRenderResultLike {
                        content: result.content,
                        details: details.as_ref(),
                        terminate: result.terminate,
                        is_error: result.is_error,
                    },
                    options,
                    theme,
                    context,
                )
            }
        }
    }
    /// Render persisted details, decoding only when a typed callback needs present details.
    ///
    /// # Errors
    /// Returns the original conversion or callback diagnostic.
    pub fn invoke_raw<F>(
        &self,
        input: (&ToolRenderResultLike<'_, Value>, ToolRenderResultOptions),
        theme: &Theme,
        context: &ToolRenderContext<'_, S>,
        decode: F,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo>
    where
        F: FnOnce(&Value) -> Result<D, DiagnosticErrorInfo>,
    {
        let (result, options) = input;
        match self {
            Self::Raw(callback) => callback(result, options, theme, context),
            Self::Typed(callback) => {
                let details = result.details.map(decode).transpose()?;
                callback(
                    &ToolRenderResultLike {
                        content: result.content,
                        details: details.as_ref(),
                        terminate: result.terminate,
                        is_error: result.is_error,
                    },
                    options,
                    theme,
                    context,
                )
            }
        }
    }
}
