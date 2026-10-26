//! Owned browser tool content and call-scoped native operands.
mod ui_error;
use crate::dom::PreparedContent;
use maestro_app::JsonObject;
use serde_json::Value;
pub use ui_error::ToolUiError;
mod binding;
mod result_view;
pub use binding::ToolRendererBinding;
pub use result_view::ToolResultMessageView;

/// Rendered content and outer-card choice.
///
/// `C` selects the content representation. By default it is page content
/// ([`crate::dom::DomContent`]) in the browser and the owned prepared record on native targets.
pub struct ToolRenderResult<C = PreparedContent> {
    /// Owned content, independent of call-scoped operands.
    pub content: C,
    /// Whether the caller should omit its ordinary tool card.
    pub is_custom: bool,
}
/// Custom renderer receiving original native operands or explicitly decoded JSON.
///
/// `C` is the produced content representation; see [`ToolRenderResult`].
pub trait ToolRenderer<P = JsonObject, D = Value, C = PreparedContent> {
    /// Prepare owned content without retaining borrowed operands.
    /// # Errors
    /// Returns the renderer's preparation failure.
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError>;
}
/// Borrowed parameters with a deferred default-rendering projection.
pub struct ToolParams<'a, P = Value> {
    /// Original caller operand.
    pub(crate) value: &'a P,
    /// Invoked only after default rendering is selected.
    pub(crate) format: fn(&P) -> Result<Option<String>, ToolUiError>,
}
impl<'a> ToolParams<'a, JsonObject> {
    /// Borrow model arguments with deferred canonical object formatting.
    #[must_use]
    pub fn object(value: &'a JsonObject) -> Self {
        Self::typed(value, |object| {
            maestro_app::pretty_object(object)
                .map(Some)
                .map_err(|error| ToolUiError::Presentation(error.to_string()))
        })
    }
}
impl<'a> ToolParams<'a, Value> {
    /// Borrow ordinary JSON using the default formatter.
    #[must_use]
    pub fn json(value: &'a Value) -> Self {
        Self::typed(value, super::renderers::default_renderer::format_params)
    }
}
impl<'a, P> ToolParams<'a, P> {
    /// Borrow a native value with its optional literal input projection.
    #[must_use]
    pub fn typed(value: &'a P, format: fn(&P) -> Result<Option<String>, ToolUiError>) -> Self {
        Self { value, format }
    }
}

/// Share whole-value truth/presence without copying an argument object.
pub(crate) fn json_params<'a>(
    value: Option<&'a Value>,
    empty: &'a JsonObject,
) -> Option<&'a JsonObject> {
    value.and_then(|value| {
        value
            .as_object()
            .or_else(|| maestro_app::is_truthy(value).then_some(empty))
    })
}
