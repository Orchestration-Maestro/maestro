//! Selected presentation and final clipping of actual native card controls.
#![cfg(test)]
use crate::{
    markdown_support::{Surface, check, publish},
    support::reading_view::view,
    tool_renderer_browser as browser,
};
use maestro_app::JsonObject;
use maestro_web::{
    BashRenderer, CalculateRenderer, ExpandableSection, ToolRenderer, tools::types::ToolRenderState,
};
use std::rc::Rc;

pub fn styles() {
    let mut surface = Surface::new();
    for pack in [surface.pack.clone(), surface.alternate.clone()] {
        for mode in [
            maestro_theme::BrandMode::Light,
            maestro_theme::BrandMode::Dark,
        ] {
            let presentation = pack.presentation(mode).unwrap();
            cards(&mut surface, &presentation);
            section(&mut surface, &presentation);
            narrow(&mut surface, &presentation);
        }
    }
}

fn cards(surface: &mut Surface, presentation: &maestro_theme::BrandPresentation<'_>) {
    let args: JsonObject = serde_json::from_value(
        serde_json::json!({"command":"literal output", "expression":"literal expression"}),
    )
    .unwrap();
    let result = crate::recipes::message::<()>(None);
    let recipe = BashRenderer
        .render(Some(&args), Some((&result).into()), false)
        .unwrap();
    let mut state = ToolRenderState::default();
    let host = surface.host();
    surface.draw_with(presentation, (400., 400.), None, |ui, frame| {
        recipe.render(ui, view(&host, frame), &mut state).unwrap()
    });
    publish("selectedMuted", &presentation.colors["muted"].into());
    publish("selectedError", &presentation.colors["error"].into());
    check(
        "(()=>{const p=document.createElement('span');p.style.color=selectedMuted;document.body.append(p);const matches=getComputedStyle(p).color===getComputedStyle(document.querySelector('.maestro-tool-caption')).color;p.remove();return matches})()",
    );
    browser::has_role(surface, browser::glyphs(surface)[0], "error", presentation);
    check(
        "(()=>{const a=document.querySelector('.maestro-tool-caption').closest('.maestro-reading-frame').getBoundingClientRect(),b=document.querySelector('.maestro-console').closest('.maestro-reading-frame').getBoundingClientRect();return Math.abs(b.top-a.bottom-12)<0.001})()",
    );
    let recipe = CalculateRenderer
        .render(Some(&args), Some((&result).into()), false)
        .unwrap();
    surface.draw_with(presentation, (400., 400.), None, |ui, frame| {
        recipe.render(ui, view(&host, frame), &mut state).unwrap()
    });
    check(
        "(()=>{const p=document.createElement('span');p.style.color=selectedError;document.body.append(p);const matches=getComputedStyle(p).color===getComputedStyle(document.querySelector('.maestro-message-error')).color;p.remove();return matches})()",
    );
    drop(state);
}
fn section(surface: &mut Surface, presentation: &maestro_theme::BrandPresentation<'_>) {
    let host = surface.host();
    let mut section = ExpandableSection::new(Rc::clone(&host), vec![]);
    section.summary = "literal summary".into();
    let response = surface.draw_with(presentation, (400., 400.), None, |ui, frame| {
        section.render(ui, view(&host, frame)).unwrap()
    });
    browser::has_role(surface, browser::glyphs(surface)[0], "muted", presentation);
    check("getComputedStyle(document.querySelector('.maestro-tool-caption')).fontSize==='14px'");
    let border = egui::Rect::from_min_max(
        response.rect.min + egui::vec2(32., 0.),
        egui::pos2(response.rect.right(), response.rect.top() + 1.),
    );
    assert!(
        browser::pixels(surface, border)
            .iter()
            .all(|pixel| pixel[3] == 0)
    );
    surface
        .context
        .memory_mut(|memory| memory.request_focus(response.id));
    surface.draw_with(presentation, (400., 400.), None, |ui, frame| {
        section.render(ui, view(&host, frame)).unwrap()
    });
    browser::has_role(surface, border, "accent", presentation);
    let glyph = browser::glyphs(surface)[0];
    let clip = egui::Rect::from_min_max(egui::pos2(glyph.center().x, 0.), egui::pos2(400., 400.));
    surface.draw_with(presentation, (400., 400.), None, |ui, frame| {
        ui.set_clip_rect(clip);
        section.render(ui, view(&host, frame)).unwrap()
    });
    let outside =
        egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(glyph.center().x - 1., 30.));
    assert!(
        browser::pixels(surface, outside)
            .iter()
            .all(|pixel| pixel[3] == 0)
    );
    assert!(
        !browser::pixels(surface, clip)
            .iter()
            .all(|pixel| pixel[3] == 0)
    );
}

fn narrow(surface: &mut Surface, presentation: &maestro_theme::BrandPresentation<'_>) {
    let host = surface.host();
    let mut section = ExpandableSection::new(host.clone(), vec![]);
    section.summary = "Narrow literal summary".into();
    for width in [20., 60.] {
        surface.draw_with(presentation, (width, 100.), None, |ui, frame| {
            section.render(ui, view(&host, frame)).unwrap()
        });
        let outside =
            egui::Rect::from_min_max(egui::pos2(width, 0.), egui::pos2(width + 20., 100.));
        assert!(
            browser::pixels(surface, outside)
                .iter()
                .all(|pixel| pixel[3] == 0)
        );
        publish("availableWidth", &f64::from(width).into());
        check(
            "[...document.querySelectorAll('.maestro-reading-frame')].every(n=>n.getBoundingClientRect().width<=availableWidth)",
        );
    }
}
