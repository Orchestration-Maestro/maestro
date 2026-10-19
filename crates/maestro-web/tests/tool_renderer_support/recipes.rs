//! Shared owned recipes and call operands for renderer witnesses.
#![cfg(test)]
use maestro_web::{
    tools::types::ToolUiError,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{
            ToolContent, ToolRenderResult, ToolRenderView, ToolRenderer, ToolResultMessageView,
        },
    },
};
use std::rc::Rc;

pub type Callback<P, D> = dyn Fn(
    Option<&P>,
    Option<ToolResultMessageView<'_, D>>,
    bool,
) -> Result<ToolRenderResult, ToolUiError>;
pub struct Renderer<P, D>(pub Rc<Callback<P, D>>);
impl<P, D> ToolRenderer<P, D> for Renderer<P, D> {
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        streaming: bool,
    ) -> Result<ToolRenderResult, ToolUiError> {
        (self.0)(params, result, streaming)
    }
}
pub struct Caption(pub String);
impl ToolContent for Caption {
    fn render(
        &self,
        ui: &mut egui::Ui,
        _: ToolRenderView<'_>,
        _: &maestro_app::ToolRendererState,
    ) -> maestro_web::tools::types::ToolContentOutput {
        let response = Ok(ui.label(&self.0));
        maestro_web::tools::types::ToolContentOutput {
            response,
            overlays: Vec::new(),
        }
    }
}
#[must_use]
pub fn content(text: &str, is_custom: bool) -> ToolRenderResult {
    ToolRenderResult {
        content: Box::new(Caption(text.into())),
        is_custom,
    }
}
pub fn reset() {
    let old = TOOL_RENDERERS.with(std::cell::RefCell::take);
    drop(old);
    maestro_web::set_show_json_mode(false);
}
pub fn message<D>(details: Option<D>) -> maestro_app::ToolResultMessage<D> {
    maestro_app::ToolResultMessage {
        tool_call_id: "original call".into(),
        tool_name: "original tool".into(),
        timestamp: 17.25,
        content: vec![
            maestro_app::UserBlock::Text(maestro_app::TextContent {
                text: "one".into(),
                text_signature: None,
            }),
            maestro_app::UserBlock::Text(maestro_app::TextContent {
                text: "two".into(),
                text_signature: None,
            }),
        ],
        details,
        is_error: true,
    }
}

/// Owned rich text drawn through the ordinary retained recipe state.
#[cfg(target_arch = "wasm32")]
pub struct RichText(pub String);
#[cfg(target_arch = "wasm32")]
impl ToolContent for RichText {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> maestro_web::tools::types::ToolContentOutput {
        let response = {
            let owner =
                state.state::<Option<maestro_web::components::markdown_block::MarkdownBlock>>();
            let mut block = owner.borrow_mut().take().unwrap_or_else(|| {
                maestro_web::components::markdown_block::MarkdownBlock::new(Rc::clone(view.host))
            });
            block.content.clone_from(&self.0);
            let result = block
                .render(
                    ui,
                    maestro_web::components::code_block::CodeBlockView {
                        i18n: view.i18n,
                        presentation: view.presentation,
                        highlighter: view.highlighter,
                        modal: view.modal,
                    },
                )
                .map_err(ToolUiError::Reading);
            *owner.borrow_mut() = Some(block);
            result
        };
        maestro_web::tools::types::ToolContentOutput {
            response,
            overlays: Vec::new(),
        }
    }
}
