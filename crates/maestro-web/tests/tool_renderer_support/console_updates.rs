//! Native console updates, localization and shared gesture admission.
#![cfg(test)]
use super::{browser, message};
use crate::{
    markdown_support::{check, first_slot_center, js, settle},
    support, surface,
};
use maestro_web::{
    ConsoleBlock, render_tool,
    tools::types::{ToolParams, ToolRenderState},
};
use serde_json::Value;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

fn scroll_up() {
    let _ = js("document.querySelector('.console-scroll').scrollTop = 0");
}
fn bottom() {
    check(
        "(()=>{const s=document.querySelector('.console-scroll'); return s.scrollTop > 0 && Math.abs(s.scrollHeight-s.clientHeight-s.scrollTop)<=1})()",
    );
}

#[wasm_bindgen]
pub async fn console_scrolls_after_updates_but_not_unchanged_frames() {
    let mut surface = surface();
    let mut console = ConsoleBlock::new(surface.host());
    let _ = js(
        "globalThis.consoleRestores=[]; const original=window.setTimeout.bind(window); window.setTimeout=(fn,delay,...args)=>{if(delay===1500){consoleRestores.push(fn);return 9000+consoleRestores.length} return original(fn,delay,...args)}",
    );
    console.content = "long output\n".repeat(90);
    browser::console_frame(&mut surface, &mut console);
    bottom();
    scroll_up();
    browser::console_frame(&mut surface, &mut console);
    check("document.querySelector('.console-scroll').scrollTop === 0");
    let pack = surface.alternate.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Light).unwrap();
    surface.draw_with(&presentation, (800., 600.), None, |ui, frame| {
        console
            .render(ui, frame.i18n, frame.presentation, None)
            .unwrap()
    });
    check("document.querySelector('.console-scroll').scrollTop === 0");
    console.content.push_str("new output");
    browser::console_frame(&mut surface, &mut console);
    bottom();
    scroll_up();
    console.variant = maestro_web::components::console_block::ConsoleVariant::Error;
    browser::console_frame(&mut surface, &mut console);
    bottom();
    scroll_up();
    let at = first_slot_center();
    surface.click(at, |ui, frame| {
        console
            .render(ui, frame.i18n, frame.presentation, None)
            .unwrap()
    });
    settle().await;
    browser::console_frame(&mut surface, &mut console);
    bottom();
    scroll_up();
    check("consoleRestores.length === 1");
    let _ = js("consoleRestores[0]()");
    browser::console_frame(&mut surface, &mut console);
    bottom();
    let _ = js(
        "globalThis.keptConsole=document.querySelector('.maestro-console'); globalThis.keptText=keptConsole.querySelector('pre').textContent",
    );
    console.content.push_str("not yet published");
    surface.effects.fail_phase.set(Some(
        maestro_web::browser::attachment_content::ReadingPhase::Publish,
    ));
    surface.draw(|ui, frame| {
        assert!(
            console
                .render(ui, frame.i18n, frame.presentation, None)
                .is_err()
        );
    });
    check("keptConsole.isConnected && keptConsole.querySelector('pre').textContent === keptText");
    browser::console_frame(&mut surface, &mut console);
    bottom();
}

#[wasm_bindgen]
pub async fn tool_renderer_uses_localization_without_translating_literal_captions() {
    let mut surface = surface();
    let language = Rc::new(support::Host::default());
    surface.i18n = maestro_web::I18n::new(language.clone()).unwrap();
    for (code, input, output, empty, label, copied, title) in [
        (
            "en",
            "Input",
            "Output",
            "(no output)",
            "console",
            "Copied!",
            "Copy output",
        ),
        (
            "de",
            "Eingabe",
            "Ausgabe",
            "(keine Ausgabe)",
            "Konsole",
            "Kopiert!",
            "Ausgabe kopieren",
        ),
    ] {
        *language.stored.borrow_mut() = Some(code.into());
        let params = serde_json::json!({"value":42});
        let mut result = message::<Value>(None);
        result.content.clear();
        let recipe = render_tool(
            "unknown",
            Some(ToolParams::json(&params)),
            Some(&result),
            false,
        )
        .unwrap();
        let mut row = ToolRenderState::default();
        browser::draw(&mut surface, &recipe, &mut row);
        crate::markdown_support::publish("localizedLabels", &format!("{input}|{output}").into());
        crate::markdown_support::publish("localizedEmpty", &empty.into());
        check(
            "[...document.querySelectorAll('.maestro-tool-label')].map(n=>n.textContent).join('|') === localizedLabels",
        );
        check("document.querySelectorAll('.maestro-code code')[1].textContent === localizedEmpty");
        check("document.querySelector('.maestro-tool-caption').textContent === 'Tool Call'");
        drop(row);
        let preparing = render_tool::<Value, Value>("unknown", None, None, false).unwrap();
        let mut row = ToolRenderState::default();
        browser::draw(&mut surface, &preparing, &mut row);
        check(
            "document.querySelector('.maestro-tool-caption').textContent === 'Preparing tool...'",
        );
        drop(row);
        localized_console(&mut surface, (label, copied, title)).await;
    }
}

async fn localized_console(
    surface: &mut crate::markdown_support::Surface,
    (label, copied, title): (&str, &str, &str),
) {
    let mut console = ConsoleBlock::new(surface.host());
    browser::console_frame(surface, &mut console);
    crate::markdown_support::publish("localizedConsole", &label.into());
    check("document.querySelector('.maestro-console-label').textContent === localizedConsole");
    let at = first_slot_center();
    surface.click(at, |ui, frame| {
        console
            .render(ui, frame.i18n, frame.presentation, None)
            .unwrap()
    });
    assert!(surface.output.iter().any(|event| matches!(event,
            egui::output::OutputEvent::Clicked(info) if info.label.as_deref() == Some(title))));
    settle().await;
    browser::console_frame(surface, &mut console);
    assert!(surface.painted.iter().any(|(text, _)| text == copied));
}

fn key(key: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]
}
#[wasm_bindgen]
pub async fn tool_controls_keep_keyboard_focus_clipped_pointer_and_reveal() {
    let mut surface = surface();
    let mut console = ConsoleBlock::new(surface.host());
    console.content = "keyboard output".into();
    let _ = js(
        "globalThis.reveals=0; const reveal=Element.prototype.scrollIntoView; Element.prototype.scrollIntoView=function(...args){reveals++; return reveal.apply(this,args)}",
    );
    browser::console_frame(&mut surface, &mut console);
    surface.events = key(egui::Key::Tab);
    browser::console_frame(&mut surface, &mut console);
    check("reveals > 0");
    surface.events = key(egui::Key::Enter);
    browser::console_frame(&mut surface, &mut console);
    settle().await;
    assert_eq!(&*surface.effects.copies.borrow(), &["keyboard output"]);
    pointer_negatives(&mut surface, &mut console);
    drop(console);
    let expanded = Rc::new(std::cell::Cell::new(false));
    let header = maestro_web::tools::types::ToolRenderResult {
        content: maestro_web::render_collapsible_header(
            maestro_web::tools::renderer_registry::ToolHeaderState::Complete,
            maestro_web::icons::icons()
                .unwrap()
                .shift_remove("Code")
                .unwrap(),
            maestro_web::tools::renderer_registry::ToolHeaderText::Text("keyboard caption".into()),
            expanded.clone(),
        ),
        is_custom: false,
    };
    let mut row = ToolRenderState::default();
    let response = browser::draw(&mut surface, &header, &mut row);
    surface
        .context
        .memory_mut(|memory| memory.request_focus(response.id));
    surface.events = key(egui::Key::Enter);
    browser::draw(&mut surface, &header, &mut row);
    assert!(expanded.get());
}

fn pointer_negatives(surface: &mut crate::markdown_support::Surface, console: &mut ConsoleBlock) {
    let center = first_slot_center();
    for (press_visible, release_visible, enabled) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        for (pressed, visible) in [(true, press_visible), (false, release_visible)] {
            surface.events = vec![
                egui::Event::PointerMoved(center),
                egui::Event::PointerButton {
                    pos: center,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            pointer_frame(surface, console, (visible, enabled));
        }
        assert_eq!(surface.effects.copies.borrow().len(), 1);
    }
    for pressed in [None, Some(true), Some(false)] {
        surface.events = pressed.map_or_else(Vec::new, |pressed| {
            vec![egui::Event::PointerButton {
                pos: center,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]
        });
        surface.draw(|ui, frame| {
            egui::Area::new(egui::Id::new("occluded-copy"))
                .order(egui::Order::Foreground)
                .fixed_pos(center - egui::vec2(20., 20.))
                .show(ui.ctx(), |ui| {
                    ui.allocate_response(egui::vec2(40., 40.), egui::Sense::hover());
                });
            console
                .render(ui, frame.i18n, frame.presentation, None)
                .unwrap()
        });
    }
    assert_eq!(surface.effects.copies.borrow().len(), 1);
}

fn pointer_frame(
    surface: &mut crate::markdown_support::Surface,
    console: &mut ConsoleBlock,
    (visible, enabled): (bool, bool),
) {
    surface.draw(|ui, frame| {
        if !visible {
            ui.set_clip_rect(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1., 1.),
            ));
        }
        if !enabled {
            ui.disable();
        }
        console
            .render(ui, frame.i18n, frame.presentation, None)
            .unwrap()
    });
}
