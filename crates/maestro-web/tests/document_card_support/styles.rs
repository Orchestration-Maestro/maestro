//! Selected brand roles and reached operation failures through real document cards.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{fixture, prepare};
use crate::{
    markdown_support::{Surface, check},
    tool_renderer_browser as drawing,
    tool_renderer_support::styles::{draw, native_color},
};
use maestro_web::{
    browser::attachment_content::AttachmentHostError,
    tools::types::{ToolRenderState, ToolUiError},
};

pub fn run() {
    let mut surface = Surface::new();
    for pack in [surface.pack.clone(), surface.alternate.clone()] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            let presentation = pack.presentation(mode).unwrap();
            for error in [false, true] {
                card(&mut surface, &presentation, error);
            }
        }
    }
    errors(&mut surface);
}
fn card(surface: &mut Surface, presentation: &maestro_theme::BrandPresentation<'_>, error: bool) {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "extract-result-valid-false-false")
        .unwrap();
    surface.time = Some(surface.time.unwrap_or(0.) + 1.);
    row.input.result.as_mut().unwrap().is_error = error;
    let recipe = prepare(&row).unwrap();
    let mut state = ToolRenderState::default();
    draw(surface, &recipe, &mut state, presentation);
    drawing::has_role(
        surface,
        drawing::glyphs(surface)[0],
        if error { "error" } else { "success" },
        presentation,
    );
    let at = drawing::glyphs(surface)[0].center();
    surface.events = crate::attachment_support::click(at);
    draw(surface, &recipe, &mut state, presentation);
    surface.time = Some(surface.time.unwrap() + 0.3);
    draw(surface, &recipe, &mut state, presentation);
    native_color(".maestro-tool-url", "muted", presentation);
    check(
        "(()=>{const u=document.querySelector('.maestro-tool-url'),s=getComputedStyle(u);return s.fontSize==='14px'&&s.lineHeight==='20px'&&u.querySelector('strong').textContent==='URL:'&&u.querySelectorAll('strong').length===1})()",
    );
    surface.events = vec![egui::Event::PointerMoved(at)];
    draw(surface, &recipe, &mut state, presentation);
    let _ = crate::markdown_support::js(
        "const caption=document.querySelector('.maestro-tool-caption');getComputedStyle(caption).color;for(const animation of caption.getAnimations())animation.finish()",
    );
    native_color(".maestro-tool-caption", "text", presentation);
}

fn errors(surface: &mut Surface) {
    use crate::support::reading_view::view;
    use std::error::Error as _;
    let row = fixture()
        .into_iter()
        .find(|r| r.id == "extract-result-valid-false-false")
        .unwrap();
    let recipe = prepare(&row).unwrap();
    let mut state = ToolRenderState::default();
    let host = surface.host();
    *surface.effects.fail_next.borrow_mut() = Some("controlled document placement".into());
    let output = surface.draw(|ui, frame| recipe.render(ui, view(&host, frame), &mut state));
    assert!(output.overlays.is_empty());
    assert!(
        matches!(output.response, Err(ToolUiError::Host(AttachmentHostError::Presentation(ref message))) if message == "controlled document placement")
    );
    drawing::draw(surface, &recipe, &mut state);
    let dictionary = surface.i18n.get_translations();
    let borrow = dictionary.borrow_mut();
    let output = surface.draw(|ui, frame| recipe.render(ui, view(&host, frame), &mut state));
    assert!(output.overlays.is_empty());
    assert!(matches!(output.response, Err(ToolUiError::I18n(_))));
    drop(borrow);
    drawing::draw(surface, &recipe, &mut state);
    let marker = std::sync::Arc::new(());
    surface.highlighter =
        std::rc::Rc::new(crate::failing_highlighter::FailingEngine(marker.clone()));
    let output = surface.draw(|ui, frame| recipe.render(ui, view(&host, frame), &mut state));
    assert!(output.overlays.is_empty());
    let error = output.response.err().unwrap();
    let cause = error
        .source()
        .unwrap()
        .source()
        .unwrap()
        .downcast_ref::<crate::failing_highlighter::Cause>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&cause.0, &marker));
}
