//! Selected presentation observed in native glyphs and final-clip card paint.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{markdown_support, renderer_recipes, support};

#[cfg(test)]
mod tests {
    use maestro_theme::{BrandMode, BrandPresentation};
    use maestro_web::{
        AssistantMessage, AssistantMessageProps, ThinkingBlock, ToolMessage, ToolMessageDebugView,
        ToolMessageProps, UserMessage, UserMessageInput,
    };
    use markdown_support::{Surface, check, js, publish};
    use serde_json::json;
    use std::rc::Rc;
    use wasm_bindgen::prelude::*;

    use super::*;

    /// All distinct text treatments use selected colors in both packs and modes.
    /// # Panics
    /// Panics if native screenshots, contrast, styling or raster paint differ.
    #[wasm_bindgen]
    pub async fn message_roles_paint_selected_pack_colors_and_decorations() {
        let mut surface = Surface::new();
        for pack in [surface.pack.clone(), surface.alternate.clone()] {
            for mode in [BrandMode::Dark, BrandMode::Light] {
                let presentation = pack.presentation(mode).unwrap();
                publish(
                    "colors",
                    &serde_json::to_string(&presentation.colors).unwrap().into(),
                );
                let _ = js("document.body.style.background=JSON.parse(colors).background");
                user_paint(&mut surface, &presentation).await;
                assistant_paint(&mut surface, &presentation).await;
                thinking_paint(&mut surface, &presentation).await;
                tool_paint(&mut surface, &presentation).await;
                debug_paint(&mut surface, &presentation).await;
                check("document.querySelectorAll('.maestro-reading-frame').length===0");
            }
        }
    }
    /// Absent output keeps body weight while both debug headings stay medium.
    /// # Panics
    /// Panics if native glyph paint or the heading treatments differ.
    #[wasm_bindgen]
    pub async fn absent_result_paints_normal_weight_without_changing_headings() {
        let mut surface = Surface::new();
        let pack = surface.pack.clone();
        let presentation = pack.presentation(BrandMode::Dark).unwrap();
        let mut row = ToolMessageDebugView::new(surface.host());
        draw(&mut surface, &presentation, |ui, view| {
            row.render(ui, view).unwrap()
        });
        support::reading_paint::observe("debug-absent-weight").await;
        check(
            "(()=>{const headings=[...document.querySelectorAll('.maestro-tool-label')]; return headings.length===2 && headings.every(n=>getComputedStyle(n).fontWeight==='500')})()",
        );
    }

    /// Native action text paints the utility curve rather than the browser default easing.
    /// # Panics
    /// Panics if the halfway-time glyph pixels differ from the recorded curve.
    #[wasm_bindgen]
    pub async fn message_action_text_paints_utility_transition_midpoint() {
        let mut surface = Surface::new();
        let pack = surface.pack.clone();
        let presentation = pack.presentation(BrandMode::Dark).unwrap();
        publish(
            "colors",
            &serde_json::to_string(&presentation.colors).unwrap().into(),
        );
        let mut row = AssistantMessage::new(surface.host());
        draw(&mut surface, &presentation, |ui, view| {
            row.render(
                ui,
                &assistant("stop"),
                AssistantMessageProps::<serde_json::Value> {
                    on_cost_click: Some(Rc::new(|| {})),
                    ..Default::default()
                },
                view,
            )
            .assert_no_overlays()
            .unwrap()
        });
        support::reading_paint::observe("message-action-transition").await;
    }

    /// Screenshot specifications name native consumers, never authored markup snapshots.
    async fn observe(selector: &str, role: &str) {
        publish(
            "messagePaint",
            &json!({"selector":selector,"role":role}).to_string().into(),
        );
        support::reading_paint::observe("message-role").await;
    }
    /// Drive the real native pointer rather than adding a synthetic hover class.
    async fn observe_hover(selector: &str) {
        publish(
            "messagePaint",
            &json!({"selector":selector,"role":"text"})
                .to_string()
                .into(),
        );
        support::reading_paint::observe("message-role-hover").await;
    }
    /// Draw a supplied row using the existing native host and chosen presentation.
    fn draw<R>(
        surface: &mut Surface,
        presentation: &BrandPresentation<'_>,
        render: impl FnOnce(&mut egui::Ui, maestro_web::tools::types::ToolRenderView<'_>) -> R,
    ) -> R {
        let host = surface.host();
        surface.draw_with(presentation, (800., 600.), None, |ui, frame| {
            render(ui, support::reading_view::view(&host, frame))
        })
    }
    /// The containing bubble paints selected gradient, border and backdrop treatment.
    async fn user_paint(surface: &mut Surface, presentation: &BrandPresentation<'_>) {
        let mut row = UserMessage::new(surface.host());
        let content = maestro_app::UserContent::Text("**User message**".into());
        for attachments in [None, Some(&[][..])] {
            draw(surface, presentation, |ui, view| {
                row.render(
                    ui,
                    UserMessageInput::WithAttachments {
                        content: &content,
                        attachments,
                    },
                    view,
                )
                .assert_no_overlays()
                .unwrap()
            });
            check(
                "(()=>{const d=document.querySelector('.maestro-user-decoration').parentElement,t=document.querySelector('.maestro-markdown').closest('.maestro-reading-frame');return (d.compareDocumentPosition(t)&Node.DOCUMENT_POSITION_FOLLOWING)!==0})()",
            );
            observe(".maestro-markdown strong", "text").await;
            support::reading_paint::observe("message-bubble").await;
        }
        let modal = egui::Id::new("bubble-neighbor");
        surface.host().set_overlay(modal, true).unwrap();
        surface.host().set_overlay(modal, false).unwrap();
        support::reading_paint::observe("message-bubble").await;
        check(
            "(()=>{const n=document.querySelector('.maestro-user-decoration'),s=getComputedStyle(n);return getComputedStyle(n.parentElement).backdropFilter==='blur(10px)' && Math.abs(parseFloat(s.borderTopWidth)*devicePixelRatio-1)<0.01 && s.transitionDuration==='0.2s'})()",
        );
    }
    /// Construct one actual assistant record with visible usage.
    fn assistant(stop: &str) -> maestro_app::AssistantMessage {
        serde_json::from_value(json!({
            "role":"assistant","content":[{"type":"text","text":"**Assistant message**"}],
            "api":"openai-completions","provider":"openai","model":"controlled",
            "usage":{"input":10,"output":20,"cacheRead":0,"cacheWrite":0,"totalTokens":30,
                "cost":{"input":0.1,"output":0.2,"cacheRead":0,"cacheWrite":0,"total":0.3}},
            "stopReason":stop,"timestamp":0,"errorMessage":"literal error"
        }))
        .unwrap()
    }
    /// Plain, actionable, error and abort native text keep their selected treatments.
    async fn assistant_paint(surface: &mut Surface, presentation: &BrandPresentation<'_>) {
        let mut row = AssistantMessage::new(surface.host());
        let message = assistant("stop");
        for clickable in [false, true] {
            let on_cost_click = clickable.then(|| Rc::new(|| {}) as Rc<dyn Fn()>);
            draw(surface, presentation, |ui, view| {
                row.render(
                    ui,
                    &message,
                    AssistantMessageProps::<serde_json::Value> {
                        on_cost_click,
                        ..Default::default()
                    },
                    view,
                )
                .assert_no_overlays()
                .unwrap()
            });
            observe(".maestro-markdown strong", "text").await;
            observe(".maestro-message-usage", "muted").await;
            if clickable {
                observe_hover(".maestro-message-usage").await;
            }
        }
        for stop in ["error", "aborted"] {
            draw(surface, presentation, |ui, view| {
                row.render(
                    ui,
                    &assistant(stop),
                    AssistantMessageProps::<serde_json::Value>::default(),
                    view,
                )
                .assert_no_overlays()
                .unwrap()
            });
            if stop == "error" {
                observe(".maestro-message-error", "error").await;
                check(
                    "getComputedStyle(document.querySelector('.maestro-message-error strong')).fontWeight==='700'",
                );
                assert_error_panel(surface, presentation);
            } else {
                observe(".maestro-message-abort", "error").await;
                check(
                    "getComputedStyle(document.querySelector('.maestro-message-abort')).fontStyle==='italic'",
                );
            }
        }
    }
    /// The destructive panel really paints the selected translucent error role.
    fn assert_error_panel(surface: &Surface, presentation: &BrandPresentation<'_>) {
        let r = js(
            "(()=>{const r=document.querySelector('.maestro-message-error').getBoundingClientRect();return [r.left,r.top]})()",
        );
        let r = js_sys::Float32Array::new(&r);
        let pixel = surface.paint.pixel(
            &surface.context,
            &surface.shapes,
            egui::pos2(r.get_index(0) - 4., r.get_index(1) + 4.),
        );
        let expected = egui::Color32::from_hex(presentation.colors["error"])
            .unwrap()
            .gamma_multiply(0.1)
            .to_array();
        assert!(
            pixel
                .into_iter()
                .zip(expected)
                .all(|(a, b)| (a - f32::from(b)).abs() < 1.)
        );
        let background = egui::Color32::from_hex(presentation.colors["background"])
            .unwrap()
            .to_array();
        let foreground = egui::Color32::from_hex(presentation.colors["error"])
            .unwrap()
            .to_array();
        let composed = std::array::from_fn(|i| {
            f64::from(pixel[i]) + f64::from(background[i]) * (1. - f64::from(pixel[3]) / 255.)
        });
        let foreground = luminance(std::array::from_fn(|i| f64::from(foreground[i])));
        let background = luminance(composed);
        assert!((foreground.max(background) + 0.05) / (foreground.min(background) + 0.05) >= 4.5);
    }
    /// WCAG relative luminance of observed, composited sRGB channels.
    fn luminance(channels: [f64; 3]) -> f64 {
        let linear = channels.map(|v| {
            let v = v / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        });
        linear
            .into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(v, w)| v * w)
            .sum()
    }
    /// Both expansion states, hover and streaming paint the selected thinking roles.
    async fn thinking_paint(surface: &mut Surface, presentation: &BrandPresentation<'_>) {
        let mut row = ThinkingBlock::new(surface.host());
        row.content = "**Reasoning message**".into();
        draw(surface, presentation, |ui, view| {
            row.render(ui, view).unwrap()
        });
        observe(".maestro-thinking-header", "muted").await;
        let _ = js("document.querySelector('.maestro-thinking-header').click()");
        draw(surface, presentation, |ui, view| {
            row.render(ui, view).unwrap()
        });
        observe(".maestro-thinking strong", "muted").await;
        check("getComputedStyle(document.querySelector('.maestro-thinking')).fontStyle==='italic'");
        observe_hover(".maestro-thinking-header").await;
        row.is_streaming = true;
        draw(surface, presentation, |ui, view| {
            row.render(ui, view).unwrap()
        });
        support::reading_paint::observe("message-thinking-shimmer").await;
    }
    /// Custom recipes bypass card paint; default recipes keep selected panel and border.
    async fn tool_paint(surface: &mut Surface, presentation: &BrandPresentation<'_>) {
        let custom = register_tool_paint();
        let call = serde_json::from_value(
            json!({"type":"toolCall","id":"paint","name":"paint","arguments":{}}),
        )
        .unwrap();
        let mut row = ToolMessage::new();
        for is_custom in [false, true] {
            custom.set(is_custom);
            let rect = draw(surface, presentation, |ui, view| {
                row.render(
                    ui,
                    ToolMessageProps::<serde_json::Value> {
                        tool_call: &call,
                        tool: None,
                        result: None,
                        pending: false,
                        aborted: false,
                        is_streaming: false,
                    },
                    view,
                )
                .assert_no_overlays()
                .unwrap()
            })
            .rect;
            observe(".maestro-markdown strong", "text").await;
            let point = egui::pos2(rect.right() - 8., rect.center().y);
            let pixel = surface
                .paint
                .pixel(&surface.context, &surface.shapes, point);
            let expected = if is_custom {
                [0; 4]
            } else {
                egui::Color32::from_hex(presentation.colors["panel"])
                    .unwrap()
                    .to_array()
            };
            assert!(
                pixel
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| (a - f32::from(b)).abs() < 1.)
            );
            if !is_custom {
                assert_card_edges(surface, presentation, rect);
            }
        }
        renderer_recipes::reset();
    }
    /// The ordinary tool registration supplies an owned rich recipe.
    fn register_tool_paint() -> Rc<std::cell::Cell<bool>> {
        use maestro_web::{
            ToolRenderer, register_tool_renderer,
            tools::types::{ToolRenderResult, ToolRendererBinding},
        };
        let custom = Rc::new(std::cell::Cell::new(false));
        let captured = custom.clone();
        let renderer: Rc<dyn ToolRenderer> =
            Rc::new(renderer_recipes::Renderer(Rc::new(move |_, _, _| {
                Ok(ToolRenderResult {
                    content: Box::new(renderer_recipes::RichText("**Tool message**".into())),
                    is_custom: captured.get(),
                })
            })));
        register_tool_renderer("paint", ToolRendererBinding::from_raw(renderer));
        custom
    }
    /// Raster output, not a declared stroke, proves the card border and shadow.
    fn assert_card_edges(
        surface: &Surface,
        presentation: &BrandPresentation<'_>,
        rect: egui::Rect,
    ) {
        let border = egui::Color32::from_hex(presentation.colors["border"])
            .unwrap()
            .to_array();
        let pixels = surface.paint.region(
            &surface.context,
            &surface.shapes,
            egui::Rect::from_min_max(
                egui::pos2(rect.center().x - 4., rect.top() - 1.),
                egui::pos2(rect.center().x + 4., rect.top() + 2.),
            ),
        );
        assert!(pixels.iter().any(|p| {
            p.iter()
                .zip(border)
                .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= 1)
        }));
        let shadow = surface.paint.pixel(
            &surface.context,
            &surface.shapes,
            egui::pos2(rect.center().x, rect.bottom() + 0.5),
        );
        assert!(shadow[3] > 0. && shadow[3] < 255.);
    }
    /// Labels, absent payloads and literal code use the selected body and mono faces.
    async fn debug_paint(surface: &mut Surface, presentation: &BrandPresentation<'_>) {
        let mut row = ToolMessageDebugView::new(surface.host());
        row.call_args = Some(json!("literal listing"));
        for has_result in [false, true] {
            row.has_result = has_result;
            row.result =
                has_result.then(|| renderer_recipes::message(Some(json!("literal details"))));
            draw(surface, presentation, |ui, view| {
                row.render(ui, view).unwrap()
            });
            observe(".maestro-tool-label", "muted").await;
            observe(".maestro-code code", "text").await;
            check(
                "(()=>{const n=document.querySelector('.maestro-code code'),s=getComputedStyle(n);return s.fontFamily.replaceAll('\"','')===getComputedStyle(n.closest('.maestro-reading-frame')).getPropertyValue('--maestro-type-mono-font').trim().replaceAll('\"','')})()",
            );
        }
    }
}
