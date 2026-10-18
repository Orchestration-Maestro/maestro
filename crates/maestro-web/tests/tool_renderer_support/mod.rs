//! Shared custom renderer recipes for browser tool dispatch witnesses.
#![cfg(test)]
use maestro_web::{
    browser::attachment_content::AttachmentUiError,
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
) -> Result<ToolRenderResult, AttachmentUiError>;
pub struct Renderer<P, D>(pub Rc<Callback<P, D>>);
impl<P, D> ToolRenderer<P, D> for Renderer<P, D> {
    fn render(
        &self,
        params: Option<&P>,
        result: Option<ToolResultMessageView<'_, D>>,
        streaming: bool,
    ) -> Result<ToolRenderResult, AttachmentUiError> {
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
    ) -> Result<egui::Response, AttachmentUiError> {
        Ok(ui.label(&self.0))
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

#[cfg(not(target_arch = "wasm32"))]
mod console_controls;

/// Read semantic observations of the recorded headers, collapse, console and forwarding cases.
/// # Panics
/// Panics if the committed observations are invalid JSON.
#[must_use]
pub fn cases() -> serde_json::Value {
    serde_json::from_str(include_str!("cases.json")).unwrap()
}
#[cfg(target_arch = "wasm32")]
pub mod browser;

#[cfg(target_arch = "wasm32")]
mod headers;

#[cfg(target_arch = "wasm32")]
mod styles;

#[cfg(target_arch = "wasm32")]
mod console_updates;
#[cfg(target_arch = "wasm32")]
mod heights;
#[cfg(target_arch = "wasm32")]
mod lifetimes;
#[cfg(target_arch = "wasm32")]
mod listings;

pub mod forwarding;
