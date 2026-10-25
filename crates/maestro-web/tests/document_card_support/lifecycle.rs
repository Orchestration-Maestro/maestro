//! Document disclosure timing, subtree replacement and independent row lifetimes.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{browser::open, fixture, prepare};
use crate::{
    markdown_support::{Surface, check, js},
    support::{reading_view::view, tool_output::TextOnlyOutput as _},
    tool_renderer_browser as drawing,
};
use maestro_web::tools::types::ToolRenderState;

pub fn transitions() {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "repl-params-valid-true")
        .unwrap();
    row.input
        .params
        .as_mut()
        .unwrap()
        .insert("code".into(), serde_json::json!("line\n".repeat(100)));
    let recipe = prepare(&row).unwrap();
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let mut state = ToolRenderState::default();
    let header = drawing::draw(&mut surface, &recipe, &mut state)
        .rect
        .height();
    let _ = js("globalThis.documentCode=document.querySelector('.maestro-code')");
    let at = drawing::glyphs(&surface)[0].center();
    drawing::click(&mut surface, at, &recipe, &mut state);
    for (time, height, margin) in [
        (0., 0., 0.),
        (0.075, 425.6, 2.83905),
        (0.15, 425.6, 9.30674),
    ] {
        surface.time = Some(time);
        let response = drawing::draw(&mut surface, &recipe, &mut state);
        assert!(
            (response.rect.height() - header - height - margin).abs() < 0.03,
            "entry at {time}: actual={}, header={header}, expected body={height}, margin={margin}, native={:?}",
            response.rect.height(),
            js("documentCode.getBoundingClientRect().height").as_f64()
        );
        check("documentCode===document.querySelector('.maestro-code')");
    }
    drawing::click(&mut surface, at, &recipe, &mut state);
    for (time, height, margin) in [
        (0.20, 425.6, 7.81032),
        (0.25, 425.6, 3.06921),
        (0.45, 0., 0.),
    ] {
        surface.time = Some(time);
        let response = drawing::draw(&mut surface, &recipe, &mut state);
        assert!(
            (response.rect.height() - header - height - margin).abs() < 0.03,
            "reverse at {time}: actual={}, header={header}, expected body={height}, margin={margin}",
            response.rect.height()
        );
        check("documentCode===document.querySelector('.maestro-code')");
    }
    drawing::click(&mut surface, at, &recipe, &mut state);
    surface.time = Some(0.75);
    let response = drawing::draw(&mut surface, &recipe, &mut state);
    assert!((response.rect.height() - header - 437.6).abs() < 0.03);
    check("documentCode===document.querySelector('.maestro-code')");
}

pub fn branches() {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "repl-params-valid-false")
        .unwrap();
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let mut state = ToolRenderState::default();
    let recipe = prepare(&row).unwrap();
    drawing::draw(&mut surface, &recipe, &mut state);
    open(&mut surface, &recipe, &mut state);
    let _ = js(
        "globalThis.pendingCode=document.querySelector('.maestro-code');globalThis.retainedCaption=document.querySelector('.maestro-tool-caption')",
    );
    row.input
        .params
        .as_mut()
        .unwrap()
        .insert("code".into(), serde_json::json!("changed literal"));
    let updated = prepare(&row).unwrap();
    drawing::draw(&mut surface, &updated, &mut state);
    check(
        "document.querySelector('.maestro-code')===pendingCode && pendingCode.querySelector('code').textContent==='changed literal'",
    );
    row.input.result = Some(crate::recipes::message::<serde_json::Value>(None));
    let completed = prepare(&row).unwrap();
    drawing::draw(&mut surface, &completed, &mut state);
    check(
        "!pendingCode.isConnected && document.querySelector('.maestro-code')!==pendingCode && !retainedCaption.isConnected && document.querySelector('.maestro-tool-caption')!==retainedCaption",
    );
    surface.events = vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    drawing::draw(&mut surface, &completed, &mut state);
    surface.time = Some(0.6);
    drawing::draw(&mut surface, &completed, &mut state);
    assert_eq!(drawing::glyphs(&surface).len(), 2);
    let _ = js(
        "globalThis.completedCaption=document.querySelector('.maestro-tool-caption');globalThis.completedCode=document.querySelector('.maestro-code')",
    );
    row.input.result.as_mut().unwrap().is_error = false;
    let second = prepare(&row).unwrap();
    drawing::draw(&mut surface, &second, &mut state);
    check(
        "completedCaption===document.querySelector('.maestro-tool-caption') && completedCode===document.querySelector('.maestro-code')",
    );
    row.input.params = None;
    let empty = prepare(&row).unwrap();
    drawing::draw(&mut surface, &empty, &mut state);
    check(
        "document.querySelector('.maestro-code')===null && !completedCaption.isConnected && document.querySelector('.maestro-tool-caption')!==completedCaption",
    );
    drop(state);
    check("!retainedCaption.isConnected");
}

pub fn independent() {
    let rows: Vec<_> = fixture()
        .into_iter()
        .filter(|r| {
            matches!(
                r.id.as_str(),
                "repl-params-valid-false" | "extract-params-valid-false"
            )
        })
        .collect();
    assert_eq!(rows.len(), 2);
    let recipes = [prepare(&rows[0]).unwrap(), prepare(&rows[1]).unwrap()];
    let mut states = [ToolRenderState::default(), ToolRenderState::default()];
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let host = surface.host();
    let mut draw = |surface: &mut Surface| {
        surface.draw(|ui, frame| {
            for (recipe, state) in recipes.iter().zip(&mut states) {
                recipe
                    .render(ui, view(&host, frame), state)
                    .assert_no_overlays()
                    .unwrap();
            }
        });
    };
    draw(&mut surface);
    check("document.querySelectorAll('.maestro-tool-caption').length===2");
    let _ = js(
        "globalThis.independentCode=document.querySelector('.maestro-code');globalThis.independentUrl=document.querySelector('.maestro-tool-url');document.querySelector('.maestro-tool-caption').click()",
    );
    surface.time = Some(0.3);
    draw(&mut surface);
    surface.time = Some(0.6);
    draw(&mut surface);
    check(
        "independentCode===document.querySelector('.maestro-code') && independentUrl===document.querySelector('.maestro-tool-url')",
    );
    assert!(drawing::glyphs(&surface).len() >= 5);
    let _ = js("document.querySelectorAll('.maestro-tool-caption')[1].click()");
    surface.time = Some(0.9);
    draw(&mut surface);
    surface.time = Some(1.2);
    draw(&mut surface);
    let _ = js(
        "const r=document.createRange();r.selectNodeContents(independentUrl.lastElementChild);getSelection().removeAllRanges();getSelection().addRange(r);globalThis.keptUrlSelection=getSelection().toString()",
    );
    check("keptUrlSelection.length>0");
    drop(std::mem::take(&mut states[0]));
    check("!independentCode.isConnected && independentUrl.isConnected");
    surface.draw(|ui, frame| {
        recipes[1]
            .render(ui, view(&host, frame), &mut states[1])
            .assert_no_overlays()
            .unwrap()
    });
    check(
        "independentUrl===document.querySelector('.maestro-tool-url') && getSelection().toString()===keptUrlSelection",
    );
    drop(states);
    check("!independentUrl.isConnected");
}

pub async fn controls() {
    let row = fixture()
        .into_iter()
        .find(|r| r.id == "repl-result-valid-false-false")
        .unwrap();
    let recipe = prepare(&row).unwrap();
    let mut state = ToolRenderState::default();
    let mut surface = Surface::new();
    surface.time = Some(0.);
    denied_controls(&mut surface, &recipe, &mut state);
    open(&mut surface, &recipe, &mut state);
    assert!(surface.context.memory(egui::Memory::focused).is_some());
    let _ = js(
        "globalThis.selectedCode=document.querySelector('.maestro-code');const r=document.createRange();r.selectNodeContents(selectedCode.querySelector('code'));getSelection().removeAllRanges();getSelection().addRange(r);globalThis.documentSelection=getSelection().toString()",
    );
    drawing::click(
        &mut surface,
        crate::markdown_support::first_slot_center(),
        &recipe,
        &mut state,
    );
    crate::markdown_support::settle().await;
    assert_eq!(
        surface.effects.copies.borrow()[0],
        row.expected.as_ref().unwrap().blocks[0].code
    );
    drawing::click(
        &mut surface,
        crate::markdown_support::slot_center(1),
        &recipe,
        &mut state,
    );
    crate::markdown_support::settle().await;
    assert_eq!(
        surface.effects.copies.borrow()[1],
        row.expected.as_ref().unwrap().console[0].content
    );
    let at = drawing::glyphs(&surface)[0].center();
    drawing::click(&mut surface, at, &recipe, &mut state);
    surface.time = Some(1.2);
    drawing::draw(&mut surface, &recipe, &mut state);
    assert_eq!(drawing::glyphs(&surface).len(), 2);
    surface.events = vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: Some(egui::Key::Space),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    drawing::draw(&mut surface, &recipe, &mut state);
    surface.time = Some(1.5);
    drawing::draw(&mut surface, &recipe, &mut state);
    assert!(drawing::glyphs(&surface).len() > 2);
    check(
        "selectedCode===document.querySelector('.maestro-code') && getSelection().toString()===documentSelection",
    );
    modal(&mut surface, &recipe, &mut state, at);
}

fn modal(
    surface: &mut Surface,
    recipe: &maestro_web::tools::types::ToolRenderResult,
    state: &mut ToolRenderState,
    header: egui::Pos2,
) {
    let host = surface.host();
    let mut overlay = maestro_web::AttachmentOverlay::open(
        crate::attachment_support::attachment(),
        None,
        host.clone(),
    );
    let mut draw = |surface: &mut Surface| {
        surface.draw(|ui, frame| {
            ui.add_enabled_ui(false, |ui| {
                recipe
                    .render(ui, view(&host, frame), state)
                    .assert_no_overlays()
                    .unwrap()
            });
            overlay
                .render(ui.ctx(), frame.i18n, frame.presentation)
                .unwrap()
        })
    };
    assert!(draw(surface));
    assert!(draw(surface));
    check("getComputedStyle(selectedCode.closest('.maestro-reading-frame')).visibility==='hidden'");
    surface.events = crate::attachment_support::click(header);
    assert!(draw(surface));
    surface.events = vec![super::escape()];
    assert!(!draw(surface));
    drop(overlay);
    surface.time = Some(1.8);
    drawing::draw(surface, recipe, state);
    check(
        "selectedCode===document.querySelector('.maestro-code') && getComputedStyle(selectedCode.closest('.maestro-reading-frame')).visibility==='visible'",
    );
    assert!(drawing::glyphs(surface).len() > 2);
}

pub fn renderer_switch() {
    let mut surface = Surface::new();
    surface.time = Some(0.);
    let mut state = ToolRenderState::default();
    let rows = fixture();
    let repl = rows
        .iter()
        .find(|r| r.id == "repl-result-valid-false-false")
        .unwrap();
    let extract = rows
        .iter()
        .find(|r| r.id == "extract-result-valid-false-false")
        .unwrap();
    let recipe = prepare(repl).unwrap();
    drawing::draw(&mut surface, &recipe, &mut state);
    open(&mut surface, &recipe, &mut state);
    let _ = js(
        "globalThis.previousToolHeader=document.querySelector('.maestro-tool-caption');globalThis.previousToolCode=document.querySelector('.maestro-code')",
    );
    let changed = prepare(extract).unwrap();
    drawing::draw(&mut surface, &changed, &mut state);
    check("!previousToolHeader.isConnected && !previousToolCode.isConnected");
    assert_eq!(drawing::glyphs(&surface).len(), 2);
}

/// Keep disabled and fully clipped headers from accepting native clicks.
fn denied_controls(
    surface: &mut Surface,
    recipe: &maestro_web::tools::types::ToolRenderResult,
    state: &mut ToolRenderState,
) {
    let host = surface.host();
    surface.draw(|ui, frame| {
        ui.add_enabled_ui(false, |ui| {
            recipe
                .render(ui, view(&host, frame), state)
                .assert_no_overlays()
                .unwrap()
        })
    });
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(surface, recipe, state);
    surface.time = Some(0.3);
    drawing::draw(surface, recipe, state);
    assert_eq!(drawing::glyphs(surface).len(), 2);
    surface.draw(|ui, frame| {
        ui.set_clip_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::ZERO,
        ));
        recipe
            .render(ui, view(&host, frame), state)
            .assert_no_overlays()
            .unwrap()
    });
    let _ = js("document.querySelector('.maestro-tool-caption').click()");
    drawing::draw(surface, recipe, state);
    surface.time = Some(0.6);
    drawing::draw(surface, recipe, state);
    assert_eq!(drawing::glyphs(surface).len(), 2);
}
