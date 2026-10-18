//! Selected-pack native styles and frame-clock spinner paint.
#![cfg(test)]
use super::browser;
use crate::{
    markdown_support::{check, first_slot_center, js},
    surface,
};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::{
    ConsoleBlock, render_header,
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText},
        types::{ToolRenderResult, ToolRenderState},
    },
};
use wasm_bindgen::prelude::*;

fn header(state: ToolHeaderState) -> ToolRenderResult {
    ToolRenderResult {
        content: render_header(
            state,
            maestro_web::icons::icons()
                .unwrap()
                .shift_remove("Code")
                .unwrap(),
            ToolHeaderText::Text("caption".into()),
        ),
        is_custom: false,
    }
}
fn draw(
    surface: &mut crate::markdown_support::Surface,
    recipe: &ToolRenderResult,
    row: &mut ToolRenderState,
    presentation: &BrandPresentation<'_>,
) {
    let host = surface.host();
    surface.draw_with(presentation, (800., 600.), None, |ui, frame| {
        recipe.render(ui, browser::view(&host, frame), row).unwrap()
    });
}
fn native_color(selector: &str, role: &str, presentation: &BrandPresentation<'_>) {
    let color = egui::Color32::from_hex(presentation.colors[role]).unwrap();
    assert_eq!(
        js(&format!(
            "getComputedStyle(document.querySelector({selector:?})).color"
        ))
        .as_string()
        .unwrap(),
        format!("rgb({}, {}, {})", color.r(), color.g(), color.b())
    );
}
#[wasm_bindgen]
pub fn tool_styles_follow_selected_pack_in_both_modes() {
    let mut surface = surface();
    for pack in [surface.pack.clone(), surface.alternate.clone()] {
        for mode in [BrandMode::Dark, BrandMode::Light] {
            let presentation = pack.presentation(mode).unwrap();
            for (state, role) in [
                (ToolHeaderState::Inprogress, "text"),
                (ToolHeaderState::Complete, "success"),
                (ToolHeaderState::Error, "error"),
            ] {
                let recipe = header(state);
                let mut row = ToolRenderState::default();
                draw(&mut surface, &recipe, &mut row, &presentation);
                browser::has_role(&surface, browser::glyphs(&surface)[0], role, &presentation);
                native_color(".maestro-tool-caption", "muted", &presentation);
            }
            console_styles(&mut surface, &presentation);
        }
    }
    spinner_phases(&mut surface);
}

fn spinner_phases(surface: &mut crate::markdown_support::Surface) {
    let mut row = ToolRenderState::default();
    let pending = header(ToolHeaderState::Inprogress);
    surface.time = Some(10.);
    browser::draw(surface, &pending, &mut row);
    let original = browser::pixels(surface, *browser::glyphs(surface).last().unwrap());
    surface.time = Some(10.0625);
    browser::draw(surface, &pending, &mut row);
    let rotated = browser::pixels(surface, *browser::glyphs(surface).last().unwrap());
    assert_ne!(
        original, rotated,
        "the symmetric loader must still move between its quarter-cycle symmetries"
    );
    for (time, angle) in [
        (10.25, std::f32::consts::FRAC_PI_2),
        (10.5, std::f32::consts::PI),
        (10.75, -std::f32::consts::FRAC_PI_2),
        (11., 0.),
    ] {
        surface.time = Some(time);
        browser::draw(surface, &pending, &mut row);
        let rect = *browser::glyphs(surface).last().unwrap();
        let pixels = browser::pixels(surface, rect);
        let expected = spinner_at(surface, rect, angle);
        assert_eq!(pixels, expected, "spinner paint at reference phase {time}");
        if time.to_bits() == 11_f64.to_bits() {
            assert_eq!(pixels, original, "full-cycle paint returns exactly");
        }
    }
    spinner_reentry(surface, &pending, &mut row, &original);
}

fn spinner_reentry(
    surface: &mut crate::markdown_support::Surface,
    pending: &ToolRenderResult,
    row: &mut ToolRenderState,
    original: &[[u8; 4]],
) {
    surface.time = Some(11.5);
    browser::draw(surface, &header(ToolHeaderState::Complete), row);
    assert_eq!(browser::glyphs(surface).len(), 1);
    surface.time = Some(12.);
    browser::draw(surface, pending, row);
    assert_eq!(
        browser::pixels(surface, *browser::glyphs(surface).last().unwrap()),
        original
    );
    let mut other = ToolRenderState::default();
    surface.time = Some(12.0625);
    let host = surface.host();
    surface.draw(|ui, frame| {
        pending
            .render(ui, browser::view(&host, frame), row)
            .unwrap();
        pending
            .render(ui, browser::view(&host, frame), &mut other)
            .unwrap();
    });
    let images = browser::glyphs(surface);
    assert_ne!(
        browser::pixels(surface, images[1]),
        browser::pixels(surface, images[3])
    );
}

/// Paint independently selected reference phases using the source catalog raster.
fn spinner_at(
    surface: &mut crate::markdown_support::Surface,
    rect: egui::Rect,
    angle: f32,
) -> Vec<[u8; 4]> {
    surface.draw(|ui, frame| {
        let node = maestro_web::icons::icons()
            .unwrap()
            .shift_remove("Loader")
            .unwrap();
        let tint = egui::Color32::from_hex(frame.presentation.colors["text"]).unwrap();
        let image = maestro_web::icons::icon(
            &node,
            maestro_web::icons::IconSize::Sm,
            tint,
            ui.ctx().pixels_per_point(),
        )
        .unwrap();
        let texture = ui.ctx().load_texture(
            "reference-spinner-phase",
            image,
            egui::TextureOptions::LINEAR,
        );
        egui::Image::new((texture.id(), rect.size()))
            .rotate(angle, egui::vec2(0.5, 0.5))
            .paint_at(ui, rect);
    });
    browser::pixels(surface, rect)
}

fn console_styles(
    surface: &mut crate::markdown_support::Surface,
    presentation: &BrandPresentation<'_>,
) {
    let mut console = ConsoleBlock::new(surface.host());
    console.content = "literal output".into();
    for (variant, role) in [
        (
            maestro_web::components::console_block::ConsoleVariant::Default,
            "text",
        ),
        (
            maestro_web::components::console_block::ConsoleVariant::Error,
            "error",
        ),
    ] {
        console.variant = variant;
        surface.draw_with(presentation, (800., 600.), None, |ui, frame| {
            console
                .render(ui, frame.i18n, frame.presentation, frame.modal)
                .unwrap()
        });
        native_color(".maestro-console pre", role, presentation);
        native_color(".maestro-console-label", "muted", presentation);
        check(
            "getComputedStyle(document.querySelector('.maestro-console pre')).lineHeight === '16px'",
        );
        check(
            "getComputedStyle(document.querySelector('.maestro-console-label')).lineHeight === '16px'",
        );
        check(
            "getComputedStyle(document.querySelector('.maestro-console pre')).whiteSpace === 'pre-wrap'",
        );
        let copy = first_slot_center();
        surface.events = vec![egui::Event::PointerMoved(copy)];
        surface.draw_with(presentation, (800., 600.), None, |ui, frame| {
            console
                .render(ui, frame.i18n, frame.presentation, frame.modal)
                .unwrap()
        });
        browser::has_role(
            surface,
            browser::glyphs(surface)[0],
            "on-accent",
            presentation,
        );
    }
}
