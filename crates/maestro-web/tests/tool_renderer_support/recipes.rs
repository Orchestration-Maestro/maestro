//! Shared owned recipes and call operands for renderer witnesses.
#![cfg(test)]
use maestro_web::{
    tools::types::ToolUiError,
    tools::{
        renderer_registry::TOOL_RENDERERS,
        types::{ToolRenderResult, ToolRenderer, ToolResultMessageView},
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
#[cfg(not(target_arch = "wasm32"))]
pub type PreparedContent = Box<dyn std::any::Any>;
#[cfg(target_arch = "wasm32")]
pub type PreparedContent = Box<dyn maestro_web::dom::DomContent>;
#[cfg(target_arch = "wasm32")]
impl maestro_web::dom::DomContent for Caption {
    fn mount(
        &self,
        view: &maestro_web::dom::DomView,
    ) -> Result<maestro_web::dom::DomRoot, ToolUiError> {
        let node = view.document.create_element("span").unwrap();
        node.set_text_content(Some(&self.0));
        Ok(maestro_web::dom::DomRoot::new(node))
    }
    fn update(
        &self,
        root: &maestro_web::dom::DomRoot,
        _: &maestro_web::dom::DomView,
    ) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(&self.0));
        Ok(())
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
            maestro_app::UserBlock::Text(maestro_app::TextContent::new("one".into())),
            maestro_app::UserBlock::Text(maestro_app::TextContent::new("two".into())),
        ],
        details,
        is_error: true,
    }
}

/// Caller-owned Markdown mounted through the published page-content contract.
#[cfg(target_arch = "wasm32")]
pub struct RichText(pub String);
#[cfg(target_arch = "wasm32")]
impl maestro_web::dom::DomContent for RichText {
    fn mount(
        &self,
        view: &maestro_web::dom::DomView,
    ) -> Result<maestro_web::dom::DomRoot, ToolUiError> {
        let mut block = maestro_web::components::markdown_block::MarkdownBlock::default();
        block.content.clone_from(&self.0);
        block.mount(view)
    }
    fn update(
        &self,
        root: &maestro_web::dom::DomRoot,
        view: &maestro_web::dom::DomView,
    ) -> Result<(), ToolUiError> {
        let mut block = maestro_web::components::markdown_block::MarkdownBlock::default();
        block.content.clone_from(&self.0);
        block.update(root, view)
    }
}
