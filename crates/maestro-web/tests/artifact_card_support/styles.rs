//! Selected native artifact paint and canvas controls inside the containing clip.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    markdown_support::{Surface, check, js, publish},
    support::{reading_paint, reading_view::view, tool_output::TextOnlyOutput as _},
    tool_renderer_browser as drawing,
    tool_renderer_support::styles::native_color,
};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::{
    ArtifactsToolRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolRenderState},
};
use serde_json::{Value, json};

/// Observe native glyph/tint paint and canvas final clipping in each selected mode.
/// # Panics
/// Panics if measured native content or painted selected colors differ.
pub async fn run() {
    let mut surface = Surface::new();
    surface.time = Some(0.);
    for pack in [surface.pack.clone(), surface.alternate.clone()] {
        for mode in [BrandMode::Dark, BrandMode::Light] {
            let presentation = pack.presentation(mode).unwrap();
            publish(
                "colors",
                &serde_json::to_string(&presentation.colors).unwrap().into(),
            );
            let _ = js(
                "document.documentElement.style.background=document.body.style.background=JSON.parse(colors).background",
            );
            for error in [false, true] {
                card(&mut surface, &presentation, error).await;
            }
            for (command, filename) in [
                ("create", false),
                ("create", true),
                ("update", false),
                ("update", true),
                ("get", false),
                ("get", true),
            ] {
                header_hover(&mut surface, &presentation, command, filename).await;
            }
        }
    }
    super::pills::hover().await;
}
fn draw(
    surface: &mut Surface,
    recipe: &ToolRenderResult,
    state: &mut ToolRenderState,
    presentation: &BrandPresentation<'_>,
    clip: Option<egui::Rect>,
) {
    let host = surface.host();
    surface.draw_with(presentation, (600., 400.), None, |ui, frame| {
        if let Some(clip) = clip {
            ui.set_clip_rect(clip);
        }
        recipe
            .render(ui, view(&host, frame), state)
            .assert_no_overlays()
            .unwrap()
    });
}
async fn card(surface: &mut Surface, presentation: &BrandPresentation<'_>, error: bool) {
    surface.events = vec![egui::Event::PointerGone];
    let params = if error {
        json!({"command":"create","filename":"literal.rs","content":"literal code"})
    } else {
        json!({"command":"update","filename":"literal.rs","old_str":"old line\ncontext","new_str":"new line\ncontext"})
    };
    let mut result = crate::recipes::message::<()>(None);
    result.is_error = error;
    let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
        &ArtifactsToolRenderer::new(None),
        Some(&params),
        Some((&result).into()),
        false,
    )
    .unwrap();
    let mut state = ToolRenderState::default();
    draw(surface, &recipe, &mut state, presentation, None);
    draw(surface, &recipe, &mut state, presentation, None);
    native_color(".maestro-tool-caption", "muted", presentation);
    native_color(".maestro-artifact-pill", "text", presentation);
    drawing::has_role(
        surface,
        drawing::glyphs(surface)[0],
        if error { "error" } else { "success" },
        presentation,
    );
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    draw(surface, &recipe, &mut state, presentation, None);
    surface.time = Some(surface.time.unwrap() + 0.3);
    draw(surface, &recipe, &mut state, presentation, None);
    draw(surface, &recipe, &mut state, presentation, None);
    check(
        "(()=>{const n=document.querySelector('.maestro-artifact-pill'),s=getComputedStyle(n);return n.textContent==='literal.rs'&&n.querySelector('svg')!==null&&s.borderStyle==='solid'&&parseFloat(s.borderWidth)>0&&s.lineHeight==='16px'})()",
    );
    style(".maestro-artifact-pill", "borderColor", "border", 100);
    style(".maestro-artifact-pill", "backgroundColor", "panel", 50);
    if error {
        native_color(".maestro-message-error", "error", presentation);
    } else {
        native_color(".maestro-diff pre", "text", presentation);
        style(".maestro-diff-added", "backgroundColor", "success", 15);
        style(".maestro-diff-removed", "backgroundColor", "error", 15);
        check(
            "document.querySelector('.maestro-diff-added').textContent==='+ new line' && document.querySelector('.maestro-diff-removed').textContent==='- old line'",
        );
    }
    publish("artifactPaintError", &error.into());
    publish("artifactPaintClip", &JsValue::NULL);
    reading_paint::observe("artifact-roles").await;
    clipped_card(surface, presentation, &recipe, &mut state).await;
    drop(state);
    check(
        "document.querySelectorAll('.maestro-artifact-pill,.maestro-diff,.maestro-message-error').length===0",
    );
}
/// Rich caption labels inherit hover; independently styled pills do not.
async fn header_hover(
    surface: &mut Surface,
    presentation: &BrandPresentation<'_>,
    command: &str,
    filename: bool,
) {
    let mut params = json!({"command":command,"content":"literal","old_str":"old","new_str":"new"});
    if filename {
        params["filename"] = "literal.rs".into();
    }
    let recipe = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
        &ArtifactsToolRenderer::new(None),
        Some(&params),
        None,
        false,
    )
    .unwrap();
    let mut state = ToolRenderState::default();
    surface.events = vec![egui::Event::PointerGone];
    draw(surface, &recipe, &mut state, presentation, None);
    draw(surface, &recipe, &mut state, presentation, None);
    native_color(".maestro-tool-caption", "muted", presentation);
    let at = drawing::glyphs(surface)[0].center();
    surface.events = vec![egui::Event::PointerMoved(at)];
    draw(surface, &recipe, &mut state, presentation, None);
    draw(surface, &recipe, &mut state, presentation, None);
    publish("hoverText", &presentation.colors["text"].into());
    reading_paint::observe("tool-header-hover").await;
    native_color(".maestro-tool-caption", "text", presentation);
    if filename {
        native_color(".maestro-artifact-pill", "text", presentation);
    }
    surface.events = vec![egui::Event::PointerGone];
    draw(surface, &recipe, &mut state, presentation, None);
    draw(surface, &recipe, &mut state, presentation, None);
    publish("hoverText", &presentation.colors["muted"].into());
    reading_paint::observe("tool-header-hover").await;
    native_color(".maestro-tool-caption", "muted", presentation);
}
use wasm_bindgen::JsValue;
/// Compare selected semantic styles independently of the feature's custom properties.
fn style(selector: &str, property: &str, role: &str, percent: u8) {
    publish(
        "artifactStyle",
        &json!([selector, property, role, percent])
            .to_string()
            .into(),
    );
    check(
        "(()=>{const [selector,property,role,percent]=JSON.parse(artifactStyle),n=document.querySelector(selector),p=document.createElement('span');p.style[property]=percent===100?JSON.parse(colors)[role]:`color-mix(in srgb, ${JSON.parse(colors)[role]} ${percent}%, transparent)`;document.body.append(p);const equal=getComputedStyle(n)[property]===getComputedStyle(p)[property];p.remove();return equal})()",
    );
}

/// Retain focus and native owners while clipping both drawing paths.
async fn clipped_card(
    surface: &mut Surface,
    presentation: &BrandPresentation<'_>,
    recipe: &ToolRenderResult,
    state: &mut ToolRenderState,
) {
    // Retain focus and native owners while the containing row clips both drawing paths.
    let at = drawing::glyphs(surface)[1].center();
    surface.events = crate::attachment_support::click(at);
    draw(surface, recipe, state, presentation, None);
    assert!(surface.context.memory(egui::Memory::focused).is_some());
    let clip = egui::Rect::from_min_max(egui::pos2(12., 0.), egui::pos2(300., 56.));
    draw(surface, recipe, state, presentation, Some(clip));
    draw(surface, recipe, state, presentation, Some(clip));
    let outside = egui::Rect::from_min_max(egui::pos2(0., 58.), egui::pos2(600., 400.));
    assert!(drawing::pixels(surface, outside).iter().all(|p| p[3] == 0));
    publish(
        "artifactPaintClip",
        &js_sys::Array::of4(&12.into(), &0.into(), &300.into(), &56.into()),
    );
    reading_paint::observe("artifact-clip").await;
}
