//! Natural rich-caption height, single invocation and bounded native scrolling.
#![cfg(test)]
use crate::{
    markdown_support::{check, js},
    surface,
    tool_renderer_support::browser,
};
use maestro_web::{
    render_header,
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText},
        types::{ToolContent, ToolRenderResult, ToolRenderState, ToolRenderView},
    },
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;
struct Child {
    height: f32,
    calls: Rc<Cell<usize>>,
    actions: Rc<Cell<usize>>,
    rect: Rc<Cell<egui::Rect>>,
}
impl ToolContent for Child {
    fn render(
        &self,
        ui: &mut egui::Ui,
        _: ToolRenderView<'_>,
        _: &maestro_app::ToolRendererState,
    ) -> Result<egui::Response, maestro_web::browser::attachment_content::AttachmentUiError> {
        self.calls.set(self.calls.get() + 1);
        let response = ui
            .allocate_exact_size(egui::vec2(80., self.height), egui::Sense::click())
            .1;
        self.rect.set(response.rect);
        ui.painter()
            .rect_filled(response.rect, 0., egui::Color32::RED);
        if response.clicked() {
            self.actions.set(self.actions.get() + 1);
        }
        Ok(response)
    }
}
#[wasm_bindgen]
pub fn rich_caption_keeps_intrinsic_heights_and_single_invocation() {
    let mut surface = surface();
    for height in [8., 16., 64.] {
        let calls = Rc::new(Cell::new(0));
        let actions = Rc::new(Cell::new(0));
        let rect = Rc::new(Cell::new(egui::Rect::NOTHING));
        let recipe = ToolRenderResult {
            content: render_header(
                ToolHeaderState::Complete,
                maestro_web::icons::icons()
                    .unwrap()
                    .shift_remove("Code")
                    .unwrap(),
                ToolHeaderText::Content(Box::new(Child {
                    height,
                    calls: calls.clone(),
                    actions: actions.clone(),
                    rect: rect.clone(),
                })),
            ),
            is_custom: false,
        };
        let mut row = ToolRenderState::default();
        let response = browser::draw(&mut surface, &recipe, &mut row);
        assert_eq!(calls.get(), 1, "one child invocation in first frame");
        let icon = browser::glyphs(&surface)[0];
        assert_eq!(rect.get().height().to_bits(), height.to_bits());
        assert_eq!(icon.height().to_bits(), 16_f32.to_bits());
        assert_eq!(
            rect.get().center().y.to_bits(),
            icon.center().y.to_bits(),
            "centred height {height}"
        );
        assert_eq!(response.rect.height().to_bits(), height.max(16.).to_bits());
        browser::click(&mut surface, rect.get().center(), &recipe, &mut row);
        assert_eq!(
            calls.get(),
            4,
            "one child invocation in each of four frames"
        );
        assert_eq!(actions.get(), 1);
    }
}
#[wasm_bindgen]
pub fn vertical_native_child_grows_horizontal_caption() {
    let mut surface = surface();
    let mut block = maestro_web::components::code_block::CodeBlock::new(surface.host());
    block.code = "caption".into();
    surface.draw(|ui, view| {
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 16.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| ui.vertical(|ui| block.render(ui, view).unwrap()),
        )
    });
    check(
        "document.querySelector('.maestro-reading-frame').getBoundingClientRect().height >= document.querySelector('.control-slot').getBoundingClientRect().height",
    );
}
#[wasm_bindgen]
pub fn horizontal_native_scroll_uses_real_clip() {
    let mut surface = surface();
    let mut block = maestro_web::components::code_block::CodeBlock::new(surface.host());
    block.code = "line\n".repeat(80);
    for frame in 0..2 {
        surface.draw(|ui, view| {
            let clip = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(350., 80.));
            ui.set_clip_rect(clip);
            ui.allocate_ui_with_layout(
                egui::vec2(350., 16.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| block.render(ui, view).unwrap(),
            )
        });
        if frame == 0 {
            let _ = js(
                "globalThis.v=document.querySelector('.maestro-reading-viewport');v.scrollTop=10.5;globalThis.old=v.scrollTop",
            );
            check("old > 0");
        } else {
            check("v.scrollTop === old && v.getBoundingClientRect().height <= 80");
        }
    }
}
#[wasm_bindgen]
pub fn horizontal_offer_does_not_cap_natural_child() {
    let mut surface = surface();
    let mut block = maestro_web::components::code_block::CodeBlock::new(surface.host());
    block.code = "line\n".repeat(80);
    surface.draw(|ui, view| {
        ui.set_max_size(egui::vec2(350., 40.));
        ui.allocate_ui_with_layout(
            egui::vec2(350., 16.),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| block.render(ui, view).unwrap(),
        );
    });
    check("document.querySelector('.maestro-reading-frame').getBoundingClientRect().height > 40");
}

#[wasm_bindgen]
pub fn native_horizontal_in_scroll_area_keeps_bounded_scroll() {
    let mut surface = surface();
    let mut block = maestro_web::components::code_block::CodeBlock::new(surface.host());
    block.code = "line\n".repeat(80);
    for frame in 0..2 {
        surface.draw(|ui, view| {
            egui::ScrollArea::vertical()
                .max_height(80.)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| ui.vertical(|ui| block.render(ui, view).unwrap()))
                })
        });
        if frame == 0 {
            let _ = js(
                "globalThis.v=document.querySelector('.maestro-reading-viewport');v.scrollTop=10.5;globalThis.old=v.scrollTop",
            );
            check("old > 0");
        } else {
            check("v.scrollTop === old && v.getBoundingClientRect().height <= 80");
        }
    }
}
