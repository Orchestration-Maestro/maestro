//! Shared real-browser tool recipe drawing and final-clip observations.
#![cfg(test)]
use crate::{markdown_support::Surface, support::tool_output::TextOnlyOutput as _};
use maestro_web::tools::types::ToolUiError;
use maestro_web::{
    components::code_block::{CodeBlock, CodeBlockView},
    tools::{
        renderer_registry::ToolHeaderState,
        types::{ToolContent, ToolRenderResult, ToolRenderState, ToolRenderView},
    },
};
use std::rc::Rc;

pub use crate::support::reading_view::view;
/// Observe the real native recipe through the shared frame.
/// # Panics
/// Panics if native setup or the expected observation fails.
pub fn draw(
    surface: &mut Surface,
    recipe: &ToolRenderResult,
    row: &mut ToolRenderState,
) -> egui::Response {
    let host = surface.host();
    surface.draw(|ui, frame| {
        recipe
            .render(ui, view(&host, frame), row)
            .assert_no_overlays()
            .unwrap()
    })
}
/// Observe the real native recipe through the shared frame.
/// # Panics
/// Panics if native setup or the expected observation fails.
pub fn click(
    surface: &mut Surface,
    at: egui::Pos2,
    recipe: &ToolRenderResult,
    row: &mut ToolRenderState,
) {
    let host = surface.host();
    surface.click(at, |ui, frame| {
        recipe
            .render(ui, view(&host, frame), row)
            .assert_no_overlays()
            .unwrap()
    });
}
/// Observe the real native recipe through the shared frame.
/// # Panics
/// Panics if native setup or the expected observation fails.
#[must_use]
pub fn status(name: &str) -> ToolHeaderState {
    match name {
        "inprogress" => ToolHeaderState::Inprogress,
        "complete" => ToolHeaderState::Complete,
        "error" => ToolHeaderState::Error,
        _ => panic!("unknown recorded header state"),
    }
}
#[must_use]
pub fn glyphs(surface: &Surface) -> Vec<egui::Rect> {
    surface
        .painted
        .iter()
        .filter(|(kind, _)| kind == "image")
        .map(|(_, rect)| *rect)
        .collect()
}
#[must_use]
pub fn pixels(surface: &Surface, rect: egui::Rect) -> Vec<[u8; 4]> {
    surface
        .paint
        .region(&surface.context, &surface.shapes, rect)
}
/// Observe the real native recipe through the shared frame.
/// # Panics
/// Panics if native setup or the expected observation fails.
pub fn has_role(
    surface: &Surface,
    rect: egui::Rect,
    role: &str,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    let color = egui::Color32::from_hex(presentation.colors[role]).unwrap();
    let expected = [color.r(), color.g(), color.b()];
    assert!(
        pixels(surface, rect).iter().any(|pixel| pixel[3] > 100
            && (0..3).all(|i| (i32::from(pixel[i]) * 255
                - i32::from(expected[i]) * i32::from(pixel[3]))
            .abs()
                < 1020)),
        "painted glyph does not use {role}"
    );
}
pub struct ListingRecipe {
    pub code: String,
}
#[derive(Default)]
struct ListingState {
    block: Option<CodeBlock>,
}
impl ToolContent for ListingRecipe {
    fn render(
        &self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> maestro_web::tools::types::ToolContentOutput {
        let response = {
            let slot = state.state::<ListingState>();
            let mut block = slot
                .borrow_mut()
                .block
                .take()
                .unwrap_or_else(|| CodeBlock::new(Rc::clone(view.host)));
            block.code.clone_from(&self.code);
            block.language = "text".into();
            let result = block.render(
                ui,
                CodeBlockView {
                    i18n: view.i18n,
                    presentation: view.presentation,
                    highlighter: view.highlighter,
                    modal: view.modal,
                },
            );
            let old = slot.borrow_mut().block.replace(block);
            drop(old);
            result.map_err(ToolUiError::Reading)
        };
        maestro_web::tools::types::ToolContentOutput {
            response,
            overlays: Vec::new(),
        }
    }
}

/// Draw the same console through the shared native frame owner.
/// # Panics
/// Panics if console publication fails.
pub fn console_frame(surface: &mut Surface, console: &mut maestro_web::ConsoleBlock) {
    surface.draw(|ui, frame| {
        console
            .render(ui, frame.i18n, frame.presentation, frame.modal)
            .unwrap()
    });
}

/// Observe source text metrics and both allocation-frame and visible-text section gaps.
/// # Panics
/// Panics if a rendered caption or subsection differs from its layout contract.
pub fn default_layout() {
    use crate::markdown_support::check;
    check(
        "(()=>{const labels=[...document.querySelectorAll('.maestro-tool-label')],codes=[...document.querySelectorAll('.maestro-code')]; return labels.every((label,index)=>{const a=label.closest('.maestro-reading-frame').getBoundingClientRect(),b=codes[index].closest('.maestro-reading-frame').getBoundingClientRect(); return Math.abs(b.top-a.bottom-4)<0.001 && (a.height===0 || b.height===0 || Math.abs(codes[index].getBoundingClientRect().top-label.getBoundingClientRect().bottom-4)<0.001)})})()",
    );
    check(
        "[...document.querySelectorAll('.maestro-tool-label')].every(n=>{const s=getComputedStyle(n);return s.fontSize==='12px'&&s.lineHeight==='16px'&&s.fontWeight==='500'})",
    );
    check(
        "getComputedStyle(document.querySelector('.maestro-tool-caption')).lineHeight === '20px'",
    );
}
