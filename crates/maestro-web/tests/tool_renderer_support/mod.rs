//! Shared custom renderer recipes for browser tool dispatch witnesses.
#![cfg(test)]
use maestro_web::{
    render_collapsible_header, render_header,
    tools::types::ToolUiError,
    tools::{
        renderer_registry::{TOOL_RENDERERS, ToolHeaderState, ToolHeaderText},
        types::{
            ToolContent, ToolRenderResult, ToolRenderView, ToolRenderer, ToolResultMessageView,
        },
    },
};
use std::{cell::Cell, rc::Rc};

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
    ) -> Result<egui::Response, ToolUiError> {
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
/// Read a recorded header status name.
/// # Panics
/// Panics if the recording names an unknown status.
#[must_use]
pub fn status(name: &str) -> ToolHeaderState {
    match name {
        "inprogress" => ToolHeaderState::Inprogress,
        "complete" => ToolHeaderState::Complete,
        "error" => ToolHeaderState::Error,
        _ => panic!("unknown recorded header state"),
    }
}
/// Build the shared code-glyph header, collapsible when it gets an expansion handle.
/// # Panics
/// Panics if the icon catalog has no code glyph.
#[must_use]
pub fn header(
    state: ToolHeaderState,
    text: ToolHeaderText,
    expanded: Option<Rc<Cell<bool>>>,
) -> ToolRenderResult {
    let icon = maestro_web::icons::icons()
        .unwrap()
        .shift_remove("Code")
        .unwrap();
    ToolRenderResult {
        content: if let Some(expanded) = expanded {
            render_collapsible_header(state, icon, text, expanded)
        } else {
            render_header(state, icon, text)
        },
        is_custom: false,
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
#[cfg(not(target_arch = "wasm32"))]
mod default_sections;
#[cfg(not(target_arch = "wasm32"))]
mod header_controls;

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
