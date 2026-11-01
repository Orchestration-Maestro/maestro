//! Command output cards.
use super::card_content::{Body, Caption, CardContent, CardOutput, field, state};
use crate::tools::{
    renderer_registry::{ToolHeaderState, get_text_output},
    types::{ToolRenderResult, ToolRenderer, ToolResultMessageView, ToolUiError},
};
use maestro_app::JsonObject;
/// Stateless command renderer.
pub struct BashRenderer;
impl<D, C: CardOutput> ToolRenderer<JsonObject, D, C> for BashRenderer {
    fn render(
        &self,
        params: Option<&JsonObject>,
        result: Option<ToolResultMessageView<'_, D>>,
        _is_streaming: bool,
    ) -> Result<ToolRenderResult<C>, ToolUiError> {
        use crate::components::console_block::ConsoleVariant;
        let state = state(result.as_ref());
        let command = field(params, "command")?;
        let (caption, body) = if let Some(command) = command {
            let output = get_text_output(result.as_ref());
            let text = if output.is_empty() {
                format!("> {command}")
            } else {
                format!("> {command}\n\n{output}")
            };
            let variant = if state == ToolHeaderState::Error {
                ConsoleVariant::Error
            } else {
                ConsoleVariant::Default
            };
            ("Running command...", Body::Console(text, variant))
        } else {
            ("Waiting for command...", Body::Empty)
        };
        Ok(CardContent {
            state,
            glyph: "SquareTerminal",
            caption: Caption::Localized(caption, String::new()),
            body,
        }
        .result())
    }
}
