//! Current-time result cards without clock or timezone operations.
use super::card_content::{Body, Caption, CardContent, CardOutput, field, state};
use crate::tools::{
    renderer_registry::get_text_output,
    types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use maestro_app::JsonObject;
/// Stateless current-time result renderer; it reads no clock.
pub struct GetCurrentTimeRenderer;
impl<D, C: CardOutput> ToolRenderer<JsonObject, D, C> for GetCurrentTimeRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, D>>,
        _is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        let state = state(result.as_ref());
        let timezone = field(params, "timezone")?;
        let (key, mut suffix) = if let Some(timezone) = timezone {
            ("Getting current time in", format!(" {timezone}"))
        } else {
            (
                if params.is_none() && result.is_none() {
                    "Getting time..."
                } else {
                    "Getting current date and time"
                },
                String::new(),
            )
        };
        let body = match result.as_ref() {
            Some(result) if result.is_error() => Body::Error(get_text_output(Some(result))),
            Some(result) => {
                suffix.push_str(": ");
                suffix.push_str(&get_text_output(Some(result)));
                Body::Empty
            }
            None => Body::Empty,
        };
        Ok(CardContent {
            state,
            glyph: "Clock",
            caption: Caption::Localized(key, suffix),
            body,
        }
        .result())
    }
}
