//! Calculation result cards without expression evaluation.
use super::card_content::{Body, Caption, CardContent, CardOutput, field, state};
use crate::tools::{
    renderer_registry::get_text_output,
    types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use maestro_app::JsonObject;
/// Stateless calculation-result renderer; it evaluates no expression.
pub struct CalculateRenderer;
impl<D, C: CardOutput> ToolRenderer<JsonObject, D, C> for CalculateRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, D>>,
        _is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let state = state(result.as_ref());
        let expression = field(params, "expression")?;
        let (caption, body) = match (expression, result.as_ref()) {
            (Some(expression), Some(result)) if result.is_error() => (
                Caption::Literal(expression.into_owned()),
                Body::Error(get_text_output(Some(result))),
            ),
            (Some(expression), Some(result)) => (
                Caption::Literal(format!("{expression} = {}", get_text_output(Some(result)))),
                Body::Empty,
            ),
            (Some(expression), None) => (
                Caption::Localized("Calculating", format!(" {expression}")),
                Body::Empty,
            ),
            (None, _) => (
                Caption::Localized(
                    if params.is_some() {
                        "Writing expression..."
                    } else {
                        "Waiting for expression..."
                    },
                    String::new(),
                ),
                Body::Empty,
            ),
        };
        Ok(CardContent {
            state,
            glyph: "Calculator",
            caption,
            body,
        }
        .result())
    }
}
