//! Captured child ownership, reconnect and activation observations.
#![cfg(test)]
use crate::{
    markdown_support::{Surface, check, js, publish},
    support::reading_view::view,
    tool_renderer_browser as browser,
};
use maestro_web::{
    BashRenderer, ExpandableSection, ToolRenderer,
    tools::types::{ToolContent, ToolRenderView, ToolUiError},
};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::JsCast as _;

pub struct Child {
    pub calls: Rc<Cell<usize>>,
    pub value: Rc<RefCell<String>>,
}
impl ToolContent for Child {
    fn render(
        &self,
        ui: &mut egui::Ui,
        frame: ToolRenderView<'_>,
        state: &maestro_app::ToolRendererState,
    ) -> Result<egui::Response, ToolUiError> {
        self.calls.set(self.calls.get() + 1);
        let params =
            serde_json::from_value(serde_json::json!({"command": *self.value.borrow()})).unwrap();
        <BashRenderer as ToolRenderer<maestro_app::JsonObject, ()>>::render(
            &BashRenderer,
            Some(&params),
            None,
            false,
        )?
        .content
        .render(ui, frame, state)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lifecycle {
    initial: bool,
    expected: Moments,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Moments {
    first: Moment,
    toggled: Moment,
    updated: Moment,
    restored: Moment,
    reconnected: Moment,
    reconnected_again: Moment,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Moment {
    text: String,
    children: usize,
    identity: bool,
    child_value: String,
    chevron: String,
}

pub fn draw(surface: &mut Surface, section: &mut ExpandableSection) -> egui::Response {
    let host = surface.host();
    surface.draw(|ui, frame| section.render(ui, view(&host, frame)).unwrap())
}
pub fn toggle(surface: &mut Surface, section: &mut ExpandableSection) {
    let header = draw(surface, section);
    let host = surface.host();
    surface.click(header.rect.center(), |ui, frame| {
        section.render(ui, view(&host, frame)).unwrap()
    });
}
fn observe(surface: &mut Surface, section: &mut ExpandableSection, expected: &Moment) {
    let response = draw(surface, section);
    surface
        .context
        .memory_mut(|memory| memory.surrender_focus(response.id));
    draw(surface, section);
    publish("summaryExpected", &expected.text.clone().into());
    check("document.querySelector('.maestro-tool-caption').textContent === summaryExpected");
    assert_eq!(
        usize::try_from(
            js_sys::Array::from(&js("document.querySelectorAll('.maestro-console')")).length()
        )
        .unwrap(),
        expected.children
    );
    assert_eq!(
        js("document.querySelector('.maestro-console')===globalThis.originalChild")
            .as_bool()
            .unwrap(),
        expected.identity
    );
    publish("childExpected", &expected.child_value.clone().into());
    if expected.children > 0 {
        assert_eq!(
            js("document.querySelector('.maestro-console pre').textContent")
                .as_string()
                .unwrap(),
            format!("> {}", expected.child_value)
        );
    }
    let name = match expected.chevron.as_str() {
        "<path d=\"m9 18 6-6-6-6\"></path>" => "ChevronRight",
        "<path d=\"m6 9 6 6 6-6\"></path>" => "ChevronDown",
        _ => panic!("unknown recorded chevron"),
    };
    let rect = browser::glyphs(surface)[0];
    let color = egui::Color32::from_hex(
        surface
            .pack
            .presentation(maestro_theme::BrandMode::Dark)
            .unwrap()
            .colors["muted"],
    )
    .unwrap();
    let (paint, shapes) = crate::canvas_paint::rotated_icon(
        &surface.context,
        rect,
        name,
        (maestro_web::icons::IconSize::Sm, color, 0.),
    );
    assert_eq!(
        browser::pixels(surface, rect)
            .iter()
            .map(|p| p[3])
            .collect::<Vec<_>>(),
        paint
            .region(&surface.context, &shapes, rect)
            .iter()
            .map(|p| p[3])
            .collect::<Vec<_>>()
    );
}
pub fn lifecycle() {
    let cases: Vec<Lifecycle> =
        serde_json::from_str(include_str!("../fixtures/expandable_sections.json")).unwrap();
    for case in cases {
        let mut surface = Surface::new();
        let calls = Rc::new(Cell::new(0));
        let value = Rc::new(RefCell::new("retained".to_owned()));
        let mut section = ExpandableSection::new(
            surface.host(),
            vec![Box::new(Child {
                calls: calls.clone(),
                value: value.clone(),
            })],
        );
        section.summary = "Summary <&> 雪".into();
        section.default_expanded = true;
        draw(&mut surface, &mut section);
        let _ = js("globalThis.originalChild=document.querySelector('.maestro-console')");
        section.default_expanded = case.initial;
        section.connected_callback();
        observe(&mut surface, &mut section, &case.expected.first);
        toggle(&mut surface, &mut section);
        observe(&mut surface, &mut section, &case.expected.toggled);
        section.summary = "Changed".into();
        section.default_expanded = !case.initial;
        *value.borrow_mut() = "edited".into();
        observe(&mut surface, &mut section, &case.expected.updated);
        toggle(&mut surface, &mut section);
        observe(&mut surface, &mut section, &case.expected.restored);
        let previous = calls.get();
        if !case.initial {
            draw(&mut surface, &mut section);
            assert_eq!(calls.get(), previous);
        }
        section.connected_callback();
        observe(&mut surface, &mut section, &case.expected.reconnected);
        section.connected_callback();
        observe(&mut surface, &mut section, &case.expected.reconnected_again);
        drop(section);
        check("document.querySelectorAll('.maestro-console,.maestro-tool-caption').length===0");
    }
}
pub async fn activation() {
    let mut surface = Surface::new();
    let calls = Rc::new(Cell::new(0));
    let mut section = ExpandableSection::new(
        surface.host(),
        vec![Box::new(Child {
            calls: calls.clone(),
            value: Rc::new(RefCell::new("child".into())),
        })],
    );
    section.summary = "Toggle".into();
    let header = draw(&mut surface, &mut section);
    assert_eq!(calls.get(), 0);
    toggle(&mut surface, &mut section);
    assert!(calls.get() > 0);
    toggle(&mut surface, &mut section);
    pointer_rejections(&mut surface, &mut section, &calls, &header);
    keyboard_activation(&mut surface, &mut section);
    native_activation(&mut surface, &mut section).await;
    independent(&mut surface, &mut section);
    focus_reveal();
}

fn pointer_rejections(
    surface: &mut Surface,
    section: &mut ExpandableSection,
    calls: &Cell<usize>,
    header: &egui::Response,
) {
    let hidden = calls.get();
    let host = surface.host();
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    surface.events = vec![button(header.rect.center(), true)];
    draw(surface, section);
    surface.events = vec![button(egui::pos2(900., 900.), false)];
    draw(surface, section);
    assert_eq!(calls.get(), hidden, "outside release admitted");
    surface.events = vec![
        button(header.rect.center(), true),
        button(header.rect.center(), false),
    ];
    surface.draw(|ui, frame| {
        ui.add_enabled_ui(false, |ui| section.render(ui, view(&host, frame)).unwrap())
    });
    assert_eq!(calls.get(), hidden, "disabled header admitted");
    surface.events = vec![
        button(header.rect.center(), true),
        button(header.rect.center(), false),
    ];
    surface.draw(|ui, frame| {
        ui.set_clip_rect(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::Vec2::ZERO,
        ));
        section.render(ui, view(&host, frame)).unwrap()
    });
    assert_eq!(calls.get(), hidden, "clipped header admitted");
}

fn independent(surface: &mut Surface, section: &mut ExpandableSection) {
    let mut other = ExpandableSection::new(
        surface.host(),
        vec![Box::new(Child {
            calls: Rc::new(Cell::new(0)),
            value: Rc::new(RefCell::new("other".into())),
        })],
    );
    other.summary = "Other".into();
    other.default_expanded = true;
    let host = surface.host();
    surface.draw(|ui, frame| {
        section.render(ui, view(&host, frame)).unwrap();
        other.render(ui, view(&host, frame)).unwrap()
    });
    check(
        "document.querySelectorAll('.maestro-console').length===1 && document.querySelector('.maestro-console pre').textContent==='> other'",
    );
    let _ = js("document.querySelectorAll('.maestro-text-action')[1].click()");
    draw(surface, &mut other);
    check("document.querySelector('.maestro-console')===null");
}
fn focus_reveal() {
    let mut surface = Surface::new();
    let mut section = ExpandableSection::new(surface.host(), vec![]);
    section.summary = "Below viewport".into();
    let host = surface.host();
    let mut draw = |surface: &mut Surface| {
        surface.draw(|ui, frame| {
            egui::ScrollArea::vertical()
                .animated(false)
                .max_height(50.)
                .show(ui, |ui| {
                    ui.add_space(100.);
                    section.render(ui, view(&host, frame)).unwrap()
                })
        })
    };
    draw(&mut surface);
    surface.events = vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    draw(&mut surface);
    assert!(
        draw(&mut surface).state.offset.y > 0.,
        "focused header was not revealed"
    );
}

async fn native_activation(surface: &mut Surface, section: &mut ExpandableSection) {
    let host = surface.host();
    surface.draw(|ui, frame| {
        ui.add_enabled_ui(false, |ui| section.render(ui, view(&host, frame)).unwrap())
    });
    wasm_bindgen_futures::JsFuture::from(
        js("globalThis.clickNativeSummary ? globalThis.clickNativeSummary() : Promise.resolve(document.querySelector('.maestro-tool-caption').click())").unchecked_into::<js_sys::Promise>(),
    )
    .await
    .unwrap();
    surface.draw(|ui, frame| {
        ui.add_enabled_ui(false, |ui| section.render(ui, view(&host, frame)).unwrap())
    });
    check("document.querySelector('.maestro-console')===null");
    let response = draw(surface, section);
    surface
        .context
        .memory_mut(|memory| memory.surrender_focus(response.id));
    wasm_bindgen_futures::JsFuture::from(
        js("globalThis.clickNativeSummary ? globalThis.clickNativeSummary() : Promise.resolve(document.querySelector('.maestro-tool-caption').click())").unchecked_into::<js_sys::Promise>(),
    )
    .await
    .unwrap();
    draw(surface, section);
    check("document.querySelector('.maestro-console')!==null");
    surface.events = vec![egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    assert!(
        draw(surface, section).has_focus(),
        "native click did not focus toggle"
    );
    check("document.querySelector('.maestro-console')===null");
}

fn keyboard_activation(surface: &mut Surface, section: &mut ExpandableSection) {
    let header = draw(surface, section);
    surface
        .context
        .memory_mut(|memory| memory.surrender_focus(header.id));
    surface.events = vec![egui::Event::Key {
        key: egui::Key::Tab,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }];
    draw(surface, section);
    assert!(
        draw(surface, section).has_focus(),
        "Tab did not reach disclosure"
    );
    for key in [egui::Key::Enter, egui::Key::Space] {
        surface.events = vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        let response = draw(surface, section);
        assert!(response.has_focus());
        surface.events = vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        draw(surface, section);
        assert_eq!(
            js("document.querySelector('.maestro-console')!==null")
                .as_bool()
                .unwrap(),
            key == egui::Key::Enter
        );
    }
}
