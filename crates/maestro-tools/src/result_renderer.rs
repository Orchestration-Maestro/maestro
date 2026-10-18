//! Result callback dispatch at typed execution and persisted JSON boundaries.
use crate::{
    ToolRenderContext, ToolRenderResultOptions, output::render_utils::ToolRenderResultLike,
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::Theme;
use maestro_tui::tui::ComponentHandle;
use serde_json::Value;

/// Draws a borrowed result with its selected details representation.
pub type ToolRenderResultCallback<D, S, E = maestro_models::Local> =
    <E as maestro_models::Execution>::Shared<<E as crate::ToolExecution>::RenderResult<D, S>>;

/// An operation-local renderer accepting fresh context for each phase.
pub(crate) type PreparedRender<'a, S> = dyn Fn(
        ToolRenderResultOptions,
        &Theme,
        &ToolRenderContext<'_, S>,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo>
    + 'a;

/// Retained result data shared across repeated rendering phases.
pub struct PreparedToolRenderResult<'a, S = crate::ToolRendererState> {
    /// Owns decoded details or borrows the original raw result.
    pub(crate) render: Box<PreparedRender<'a, S>>,
}
impl<'a, S: 'a> PreparedToolRenderResult<'a, S> {
    /// Render with the supplied current options and row context.
    ///
    /// # Errors
    /// Returns the original callback diagnostic.
    pub fn render(
        &self,
        options: ToolRenderResultOptions,
        theme: &Theme,
        context: &ToolRenderContext<'_, S>,
    ) -> Result<ComponentHandle, DiagnosticErrorInfo> {
        (self.render)(options, theme, context)
    }
    /// Retain a raw callback without copying its borrowed inputs.
    pub(crate) fn raw<E: crate::ToolExecution>(
        callback: ToolRenderResultCallback<Value, S, E>,
        result: ToolRenderResultLike<'a, Value>,
    ) -> Self {
        Self {
            render: Box::new(move |options, theme, context| {
                callback(&result, options, theme, context)
            }),
        }
    }
}

/// One result slot, retaining custom typed callbacks or borrowed JSON selection.
pub enum ToolRenderResult<D, S, E: crate::ToolExecution = maestro_models::Local> {
    /// A caller-defined details contract.
    Typed(ToolRenderResultCallback<D, S, E>),
    /// A renderer that selects only the JSON fields it consumes.
    Raw(ToolRenderResultCallback<Value, S, E>),
}
impl<D, S, E: crate::ToolExecution> Clone for ToolRenderResult<D, S, E> {
    fn clone(&self) -> Self {
        match self {
            Self::Typed(callback) => Self::Typed(callback.clone()),
            Self::Raw(callback) => Self::Raw(callback.clone()),
        }
    }
}
impl<D, S, E: crate::ToolExecution> ToolRenderResult<D, S, E> {
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
        self.prepare_raw(
            ToolRenderResultLike {
                content: result.content,
                details: result.details,
                terminate: result.terminate,
                is_error: result.is_error,
            },
            decode,
        )?
        .render(options, theme, context)
    }
    /// Decode present typed details once, retaining them for repeated rendering.
    ///
    /// # Errors
    /// Returns the original conversion diagnostic without invoking the callback.
    pub fn prepare_raw<'a, F>(
        &self,
        result: ToolRenderResultLike<'a, Value>,
        decode: F,
    ) -> Result<PreparedToolRenderResult<'a, S>, DiagnosticErrorInfo>
    where
        D: 'a,
        S: 'a,
        F: FnOnce(&Value) -> Result<D, DiagnosticErrorInfo>,
    {
        match self {
            Self::Raw(callback) => Ok(PreparedToolRenderResult::raw::<E>(callback.clone(), result)),
            Self::Typed(callback) => {
                let details = result.details.map(decode).transpose()?;
                let callback = callback.clone();
                Ok(PreparedToolRenderResult {
                    render: Box::new(move |options, theme, context| {
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
                    }),
                })
            }
        }
    }
}
