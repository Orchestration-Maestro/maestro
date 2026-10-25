//! Reasoning disclosure through the actual native reading consumer.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{canvas_paint, markdown_support, support};

use maestro_web::ThinkingBlock;
use markdown_support::{Surface, check, js};
use wasm_bindgen::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// The retained chevron paints the utility transition before reaching its endpoint.
    /// # Panics
    /// Panics if opening or hover paint jumps to the destination or fails to settle.
    #[wasm_bindgen]
    pub fn thinking_chevron_paints_intermediate_rotation_and_color() {
        let mut surface = Surface::new();
        surface.time = Some(0.);
        let mut row = ThinkingBlock::new(surface.host());
        draw_thinking(&mut surface, &mut row, None);
        let glyph = surface
            .painted
            .iter()
            .find(|(name, _)| name == "image")
            .unwrap()
            .1;
        let initial = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        surface.events = vec![egui::Event::PointerMoved(glyph.center())];
        draw_thinking(&mut surface, &mut row, None);
        let started = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert_eq!(
            started, initial,
            "transition starts at the previous painted state"
        );
        surface.time = Some(0.075);
        draw_thinking(&mut surface, &mut row, None);
        let middle = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        let (reference, shapes) = canvas_paint::rotated_icon(
            &surface.context,
            glyph,
            "ChevronRight",
            (
                maestro_web::icons::IconSize::Sm,
                egui::Color32::from_rgb(236, 216, 194),
                69.8005_f32.to_radians(),
            ),
        );
        assert_eq!(
            middle,
            reference.region(&surface.context, &shapes, glyph),
            "half-time glyph matches the reference rotation and color"
        );
        surface.time = Some(0.15);
        draw_thinking(&mut surface, &mut row, None);
        let end = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert_ne!(middle, initial);
        assert_ne!(
            middle, end,
            "half-time paint has not jumped to the endpoint"
        );
        assert_ne!(end, initial);
    }

    /// Content updates keep expansion, while collapse releases the body.
    /// # Panics
    /// Panics if rendering or a native observation fails.
    #[wasm_bindgen]
    pub fn thinking_rows_keep_disclosure_state_through_streaming() {
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = ThinkingBlock::new(host.clone());
        row.content = "**initial**".into();
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking-header').textContent === 'Thinking...'");
        check("document.querySelector('.maestro-thinking') === null");
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking strong').textContent === 'initial'");
        row.content = "**streaming**".into();
        row.is_streaming = true;
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking strong').textContent === 'streaming'");
        check(
            "document.querySelector('.maestro-thinking-header').classList.contains('maestro-shimmer')",
        );
        row.is_streaming = false;
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check(
            "!document.querySelector('.maestro-thinking-header').classList.contains('maestro-shimmer')",
        );
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking') === null");
        drop(row);
        let mut row = ThinkingBlock::new(host.clone());
        surface.draw(|ui, frame| {
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking') === null");
    }
    /// Two emitted disclosures keep independent state and release saved native targets.
    /// # Panics
    /// Panics if expansion, modal admission or released-target ownership differs.
    #[wasm_bindgen]
    pub fn thinking_rows_remain_independent_and_release_native_callbacks() {
        use maestro_web::AssistantMessage;
        let mut surface = Surface::new();
        let host = surface.host();
        let mut row = AssistantMessage::new(host.clone());
        let mut message:maestro_app::AssistantMessage=serde_json::from_value(serde_json::json!({
            "role":"assistant","content":[{"type":"thinking","thinking":"**first**"},{"type":"thinking","thinking":"**second**"}],
            "api":"openai-completions","provider":"openai","model":"controlled",
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0
        })).unwrap();
        draw_assistant(&mut surface, &mut row, &message);
        let _ = js(
            "globalThis.headers=[...document.querySelectorAll('.maestro-thinking-header')]; headers[0].click()",
        );
        draw_assistant(&mut surface, &mut row, &message);
        check(
            "document.querySelectorAll('.maestro-thinking').length===1 && document.querySelector('.maestro-thinking strong').textContent==='first'",
        );
        let _ = js("headers[1].click()");
        draw_assistant(&mut surface, &mut row, &message);
        check("document.querySelectorAll('.maestro-thinking').length===2");
        let modal = egui::Id::new("neighboring-modal");
        host.set_overlay(modal, true).unwrap();
        check(
            "headers.every(h=>getComputedStyle(h.closest('.maestro-reading-frame')).visibility==='hidden')",
        );
        host.set_overlay(modal, false).unwrap();
        check(
            "headers.every(h=>getComputedStyle(h.closest('.maestro-reading-frame')).visibility==='visible')",
        );
        let maestro_app::AssistantContent::Thinking(ref mut first) = message.content[0] else {
            unreachable!()
        };
        first.thinking = "**updated**".into();
        draw_assistant(&mut surface, &mut row, &message);
        check(
            "headers[0]===document.querySelector('.maestro-thinking-header') && [...document.querySelectorAll('.maestro-thinking')].sort((a,b)=>a.getBoundingClientRect().top-b.getBoundingClientRect().top)[0].querySelector('strong').textContent==='updated'",
        );
        message.content.clear();
        draw_assistant(&mut surface, &mut row, &message);
        check(
            "headers.every(h=>!h.isConnected) && document.querySelector('.maestro-thinking')===null",
        );
        let _ = js("headers.forEach(h=>h.click())");
        drop(row);
        let mut new_row = ThinkingBlock::new(host.clone());
        new_row.content = "must remain collapsed".into();
        surface.draw(|ui, frame| {
            new_row
                .render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking')===null");
        let _ = js("headers.forEach(h=>h.click())");
        surface.draw(|ui, frame| {
            new_row
                .render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        });
        check("document.querySelector('.maestro-thinking')===null");
    }
    /// Draw one containing assistant through the actual native host.
    fn draw_assistant(
        surface: &mut Surface,
        row: &mut maestro_web::AssistantMessage,
        message: &maestro_app::AssistantMessage,
    ) {
        let host = surface.host();
        surface.draw(|ui, frame| {
            row.render(
                ui,
                message,
                maestro_web::AssistantMessageProps::<serde_json::Value>::default(),
                support::reading_view::view(&host, frame),
            )
            .assert_no_overlays()
            .unwrap()
        });
    }
    /// Draw current header geometry, optionally intersected with a caller clip.
    fn draw_thinking(
        surface: &mut Surface,
        row: &mut ThinkingBlock,
        clip: Option<egui::Rect>,
    ) -> egui::Response {
        let host = surface.host();
        surface.draw(|ui, frame| {
            if let Some(clip) = clip {
                ui.set_clip_rect(clip);
            }
            row.render(ui, support::reading_view::view(&host, frame))
                .unwrap()
        })
    }
    /// Deliver both endpoints through the current clipped canvas route.
    fn thinking_gesture(
        surface: &mut Surface,
        row: &mut ThinkingBlock,
        clip: egui::Rect,
        points: [egui::Pos2; 2],
    ) {
        for (pos, pressed) in [(points[0], true), (points[1], false)] {
            surface.events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            draw_thinking(surface, row, Some(clip));
        }
    }
    /// Native paint and rasterized glyphs follow expansion, hover, shimmer and current clipping.
    /// # Panics
    /// Panics if native screenshots, raster paint or pointer admission differs.
    #[wasm_bindgen]
    pub async fn thinking_headers_paint_shimmer_and_chevron_through_the_final_clip() {
        let mut surface = Surface::new();
        let mut row = ThinkingBlock::new(surface.host());
        row.content = "**reasoning**".into();
        let colors = surface
            .pack
            .presentation(maestro_theme::BrandMode::Dark)
            .unwrap()
            .colors;
        markdown_support::publish("colors", &serde_json::to_string(&colors).unwrap().into());
        let response = draw_thinking(&mut surface, &mut row, None);
        let glyph = egui::Rect::from_min_size(response.rect.min, egui::vec2(16., 16.));
        let collapsed = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert!(collapsed.iter().any(|p| p[3] > 0));
        support::reading_paint::observe("message-thinking-plain").await;
        surface.events = vec![egui::Event::PointerMoved(glyph.center())];
        draw_thinking(&mut surface, &mut row, None);
        support::reading_paint::observe("message-thinking-hover").await;
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        draw_thinking(&mut surface, &mut row, None);
        surface.time = Some(1.);
        draw_thinking(&mut surface, &mut row, None);
        let expanded = surface
            .paint
            .region(&surface.context, &surface.shapes, glyph);
        assert_ne!(
            expanded, collapsed,
            "rotated chevron paints a different direction"
        );
        row.is_streaming = true;
        draw_thinking(&mut surface, &mut row, None);
        check(
            "(()=>{const n=document.querySelector('.maestro-thinking-header'),s=getComputedStyle(n);return s.animationDuration==='2s' && s.animationTimingFunction==='ease-in-out' && n.style.color===''})()",
        );
        support::reading_paint::observe("message-thinking-shimmer").await;
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        row.is_streaming = false;
        draw_thinking(&mut surface, &mut row, None);
        assert_current_clip(&mut surface, &mut row, response.rect);
    }
    /// Stale geometry and mismatched gesture endpoints cannot open the disclosure.
    fn assert_current_clip(surface: &mut Surface, row: &mut ThinkingBlock, header: egui::Rect) {
        let clip = egui::Rect::from_min_size(
            header.min + egui::vec2(0.25, 0.5),
            egui::vec2(8.5, header.height() - 1.),
        );
        let inside = clip.center();
        let outside = egui::pos2(clip.right() + 2., inside.y);
        thinking_gesture(surface, row, clip, [outside, outside]);
        check("document.querySelector('.maestro-thinking')===null");
        thinking_gesture(surface, row, clip, [inside, outside]);
        check("document.querySelector('.maestro-thinking')===null");
        thinking_gesture(surface, row, clip, [inside, inside]);
        check("document.querySelector('.maestro-thinking strong').textContent==='reasoning'");
        let samples = surface
            .paint
            .region(&surface.context, &surface.shapes, header);
        assert!(samples.iter().any(|p| p[3] > 0));
        let outside_pixels = surface.paint.region(
            &surface.context,
            &surface.shapes,
            egui::Rect::from_min_max(egui::pos2(clip.right() + 1., clip.top()), header.max),
        );
        assert!(outside_pixels.iter().all(|p| p[3] == 0));
    }
}
