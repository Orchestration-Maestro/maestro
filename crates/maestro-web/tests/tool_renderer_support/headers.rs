//! Shared headers observed through native captions and final-clip glyph pixels.
#![cfg(test)]
use super::{
    browser::{self, ListingRecipe},
    cases,
};
use crate::support::tool_output::TextOnlyOutput as _;
use crate::{
    markdown_support::{check, first_slot_center, settle},
    tool_renderers_browser::surface,
};
use maestro_web::{
    render_collapsible_header, render_header,
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText},
        types::{ToolRenderResult, ToolRenderState},
    },
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;

fn recipe(
    state: ToolHeaderState,
    rich: bool,
    expanded: Option<Rc<Cell<bool>>>,
    text: &str,
) -> ToolRenderResult {
    let icon = maestro_web::icons::icons()
        .unwrap()
        .shift_remove("Code")
        .unwrap();
    let text = if rich {
        ToolHeaderText::Content(Box::new(ListingRecipe { code: text.into() }))
    } else {
        ToolHeaderText::Text(text.into())
    };
    ToolRenderResult {
        content: if let Some(expanded) = expanded {
            render_collapsible_header(state, icon, text, expanded)
        } else {
            render_header(state, icon, text)
        },
        is_custom: false,
    }
}
fn alpha(pixels: Vec<[u8; 4]>) -> Vec<u8> {
    pixels.into_iter().map(|pixel| pixel[3]).collect()
}
fn selected(images: &[egui::Rect], count: usize) -> Vec<egui::Rect> {
    match count {
        1 => vec![images[0]],
        2 => vec![images[0], *images.last().unwrap()],
        3 => vec![images[0], images[1], *images.last().unwrap()],
        _ => panic!("unexpected header icon count"),
    }
}
#[wasm_bindgen]
pub async fn tool_headers_keep_status_icon_order_and_rich_caption() {
    let mut surface = surface();
    surface.time = Some(10.);
    let cases = cases();
    let mut tool = None;
    let mut loader = None;
    for case in cases["headers"].as_array().unwrap() {
        plain_case(&mut surface, case, &mut tool, &mut loader).await;
    }

    for state in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        let recipe = recipe(state, false, Some(Rc::new(Cell::new(false))), "caption");
        let mut row = ToolRenderState::default();
        browser::draw(&mut surface, &recipe, &mut row);
        let icons = browser::glyphs(&surface);
        if state == ToolHeaderState::Inprogress {
            assert_eq!(
                alpha(browser::pixels(&surface, icons[0])),
                *loader.as_ref().unwrap()
            );
            assert_eq!(
                alpha(browser::pixels(&surface, icons[1])),
                *tool.as_ref().unwrap()
            );
            assert!(icons[0].right() < icons[1].left());
        } else {
            assert_eq!(
                alpha(browser::pixels(&surface, icons[0])),
                *tool.as_ref().unwrap()
            );
        }
    }
    let square = maestro_web::icons::icons()
        .unwrap()
        .shift_remove("Square")
        .unwrap();
    let recipe = ToolRenderResult {
        content: render_header(
            ToolHeaderState::Complete,
            square,
            ToolHeaderText::Text("custom geometry".into()),
        ),
        is_custom: false,
    };
    let mut row = ToolRenderState::default();
    browser::draw(&mut surface, &recipe, &mut row);
    assert_ne!(
        alpha(browser::pixels(&surface, browser::glyphs(&surface)[0])),
        tool.unwrap()
    );
}

#[wasm_bindgen]
pub fn collapsible_tool_header_toggles_both_initial_states() {
    let mut surface = surface();
    surface.time = Some(10.);
    for case in cases()["collapsible"].as_array().unwrap() {
        collapse_case(&mut surface, case);
    }

    independent_sections(&mut surface);
}

async fn plain_case(
    surface: &mut crate::markdown_support::Surface,
    case: &serde_json::Value,
    tool: &mut Option<Vec<u8>>,
    loader: &mut Option<Vec<u8>>,
) {
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let rich = case["rich"].as_bool().unwrap();
    let expected = &case["expected"];
    let recipe = recipe(
        browser::status(case["state"].as_str().unwrap()),
        rich,
        None,
        expected["text"].as_str().unwrap(),
    );
    let mut row = ToolRenderState::default();
    browser::draw(surface, &recipe, &mut row);
    let images = browser::glyphs(surface);
    let roles = expected["iconRoles"].as_array().unwrap();
    assert_eq!(images.len(), roles.len() + usize::from(rich));
    let icons = selected(&images, roles.len());
    for (rect, role) in icons.iter().zip(roles) {
        browser::has_role(surface, *rect, role.as_str().unwrap(), &presentation);
    }
    let code_alpha = alpha(browser::pixels(surface, icons[0]));
    if let Some(tool) = tool.as_ref() {
        assert_eq!(&code_alpha, tool);
    } else {
        *tool = Some(code_alpha);
    }
    let spinner = expected["spinner"].as_u64().unwrap();
    if spinner == 1 {
        assert!(icons[0].right() < icons[1].left());
        let pixels = alpha(browser::pixels(surface, icons[1]));
        assert_ne!(pixels, *tool.as_ref().unwrap());
        *loader = Some(pixels);
    } else {
        assert_eq!(roles.len(), 1);
    }
    caption(surface, &recipe, &mut row, (rich, expected)).await;
    drop(row);
}

async fn caption(
    surface: &mut crate::markdown_support::Surface,
    recipe: &ToolRenderResult,
    row: &mut ToolRenderState,
    (rich, expected): (bool, &serde_json::Value),
) {
    if rich {
        assert_eq!(expected["richCount"], 1);
        check("document.querySelectorAll('.maestro-code').length === 1");
        crate::markdown_support::publish(
            "expectedCaption",
            &expected["text"].as_str().unwrap().into(),
        );
        check("document.querySelector('.maestro-code code').textContent === expectedCaption");
        browser::click(surface, first_slot_center(), recipe, row);
        settle().await;
        assert_eq!(
            surface.effects.copies.borrow().last().map(String::as_str),
            expected["text"].as_str()
        );
    } else {
        assert_eq!(expected["richCount"], 0);
        check("document.querySelector('.maestro-tool-caption').textContent === 'caption'");
    }
}

fn collapse_case(surface: &mut crate::markdown_support::Surface, case: &serde_json::Value) {
    let expanded = Rc::new(Cell::new(case["expanded"].as_bool().unwrap()));
    let state = browser::status(case["state"].as_str().unwrap());
    let header = recipe(state, true, Some(expanded.clone()), "caption");
    let mut row = ToolRenderState::default();
    let mut previous = None;
    for phase in ["initial", "toggled", "restored"] {
        let response = browser::draw(surface, &header, &mut row);
        let expected = &case["expected"][phase];
        assert_eq!(expanded.get(), expected["expanded"].as_bool().unwrap());
        assert_eq!(expanded.get(), !expected["upHidden"].as_bool().unwrap());
        assert_eq!(!expanded.get(), !expected["downHidden"].as_bool().unwrap());
        assert_eq!(expected["caption"], "caption");
        assert_eq!(expected["richCount"], 1);
        check("document.querySelector('.maestro-code code').textContent === 'caption'");
        let images = browser::glyphs(surface);
        let roles = expected["iconRoles"].as_array().unwrap();
        assert_eq!(images.len(), roles.len() + 1);
        assert_eq!(
            expected["spinner"].as_u64().unwrap(),
            u64::from(state == ToolHeaderState::Inprogress)
        );
        let pack = surface.pack.clone();
        let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
        for (rect, role) in selected(&images, roles.len()).iter().zip(roles) {
            browser::has_role(surface, *rect, role.as_str().unwrap(), &presentation);
        }
        let chevron = alpha(browser::pixels(surface, *images.last().unwrap()));
        if let Some(previous) = previous {
            assert_ne!(previous, chevron);
        }
        previous = Some(chevron);
        if phase != "restored" {
            browser::click(
                surface,
                egui::pos2(response.rect.right() - 8., response.rect.center().y),
                &header,
                &mut row,
            );
        }
    }
}

fn independent_sections(surface: &mut crate::markdown_support::Surface) {
    let first = Rc::new(Cell::new(false));
    let second = Rc::new(Cell::new(true));
    let headers = [
        recipe(
            ToolHeaderState::Complete,
            false,
            Some(first.clone()),
            "first",
        ),
        recipe(
            ToolHeaderState::Complete,
            false,
            Some(second.clone()),
            "second",
        ),
    ];
    let mut rows = [ToolRenderState::default(), ToolRenderState::default()];
    let draw = |surface: &mut crate::markdown_support::Surface, rows: &mut [ToolRenderState; 2]| {
        let host = surface.host();
        surface.draw(|ui, frame| {
            let top = headers[0]
                .render(ui, browser::view(&host, frame), &mut rows[0])
                .assert_no_overlays()
                .unwrap();
            let bottom = headers[1]
                .render(ui, browser::view(&host, frame), &mut rows[1])
                .assert_no_overlays()
                .unwrap();
            [top, bottom]
        })
    };
    let responses = draw(surface, &mut rows);
    check(
        "[...document.querySelectorAll('.maestro-tool-caption')].map(n=>n.textContent).join('|') === 'first|second'",
    );
    for (index, wanted) in [(0, (true, true)), (1, (true, false))] {
        let host = surface.host();
        surface.click(responses[index].rect.center(), |ui, frame| {
            headers[0]
                .render(ui, browser::view(&host, frame), &mut rows[0])
                .assert_no_overlays()
                .unwrap();
            headers[1]
                .render(ui, browser::view(&host, frame), &mut rows[1])
                .assert_no_overlays()
                .unwrap()
        });
        assert_eq!((first.get(), second.get()), wanted);
    }
}

/// Every collapsible status exposes hover, cursor and visible keyboard focus.
#[wasm_bindgen]
pub async fn collapsible_headers_paint_interaction_states() {
    use crate::markdown_support::{js, publish};
    let mut surface = surface();
    surface.time = Some(10.);
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    for state in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        let header = recipe(state, false, Some(Rc::new(Cell::new(false))), "caption");
        let mut row = ToolRenderState::default();
        let response = browser::draw(&mut surface, &header, &mut row);
        surface.events = vec![egui::Event::PointerMoved(response.rect.center())];
        browser::draw(&mut surface, &header, &mut row);
        browser::draw(&mut surface, &header, &mut row);
        publish("hoverText", &presentation.colors["text"].into());
        let _ = js(
            "globalThis.expectedHover=document.createElement('span'); expectedHover.style.color=hoverText; document.body.appendChild(expectedHover)",
        );
        check(
            "getComputedStyle(document.querySelector('.maestro-tool-caption')).color === getComputedStyle(expectedHover).color",
        );
        check("(expectedHover.remove(),true)");
        crate::support::reading_paint::observe("tool-header-hover").await;
        assert_eq!(surface.cursor, egui::CursorIcon::PointingHand);
        surface.events = vec![egui::Event::PointerGone];
        surface
            .context
            .memory_mut(|memory| memory.surrender_focus(response.id));
        browser::draw(&mut surface, &header, &mut row);
        let icons = browser::glyphs(&surface);
        let border = egui::Rect::from_min_max(
            egui::pos2(icons[0].right() + 2., response.rect.top()),
            egui::pos2(icons.last().unwrap().left() - 2., response.rect.top() + 2.),
        );
        let unfocused = browser::pixels(&surface, border);
        surface
            .context
            .memory_mut(|memory| memory.request_focus(response.id));
        let focused = browser::draw(&mut surface, &header, &mut row);
        assert!(focused.has_focus());
        assert_ne!(browser::pixels(&surface, border), unfocused);
        browser::has_role(&surface, border, "accent", &presentation);
        drop(row);
        rich_hover_keeps_child_color(&mut surface, state).await;
    }
}

/// Nested SDK captions inherit their containing disclosure, then return to plain color.
#[wasm_bindgen]
pub async fn rich_header_labels_inherit_hover_without_frame_leak() {
    use crate::markdown_support::publish;
    let surface = &mut surface();
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let child = recipe(ToolHeaderState::Complete, false, None, "nested label");
    let icon = maestro_web::icons::icons()
        .unwrap()
        .shift_remove("Code")
        .unwrap();
    let header = ToolRenderResult {
        content: render_collapsible_header(
            ToolHeaderState::Complete,
            icon,
            ToolHeaderText::Content(child.content),
            Rc::new(Cell::new(false)),
        ),
        is_custom: false,
    };
    let mut row = ToolRenderState::default();
    surface.events = vec![egui::Event::PointerGone];
    let response = browser::draw(surface, &header, &mut row);
    surface.events = vec![egui::Event::PointerMoved(response.rect.center())];
    browser::draw(surface, &header, &mut row);
    browser::draw(surface, &header, &mut row);
    publish("hoverText", &presentation.colors["text"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    surface.events = vec![egui::Event::PointerGone];
    browser::draw(surface, &header, &mut row);
    browser::draw(surface, &header, &mut row);
    publish("hoverText", &presentation.colors["muted"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    drop(row);
    for status in [
        ToolHeaderState::Inprogress,
        ToolHeaderState::Complete,
        ToolHeaderState::Error,
    ] {
        rich_hover_keeps_child_color(surface, status).await;
    }
}

/// Independently styled rich children do not inherit the header's hover color.
async fn rich_hover_keeps_child_color(
    surface: &mut crate::markdown_support::Surface,
    state: ToolHeaderState,
) {
    use crate::markdown_support::js;
    let header = recipe(state, true, Some(Rc::new(Cell::new(false))), "rich literal");
    let mut row = ToolRenderState::default();
    surface.events = vec![egui::Event::PointerGone];
    let response = browser::draw(surface, &header, &mut row);
    let _ = js(
        "globalThis.richIdleColor=getComputedStyle(document.querySelector('.maestro-code code')).color",
    );
    crate::support::reading_paint::observe("rich-code-idle").await;
    surface.events = vec![egui::Event::PointerMoved(response.rect.center())];
    browser::draw(surface, &header, &mut row);
    browser::draw(surface, &header, &mut row);
    check("getComputedStyle(document.querySelector('.maestro-code code')).color === richIdleColor");
    crate::support::reading_paint::observe("rich-code-hover").await;
}

/// A retained literal caption loses hover color when its header becomes plain.
#[wasm_bindgen]
pub async fn retained_header_same_caption_restores_plain_color() {
    retained_header_transition("caption").await;
}

/// Editing the caption reconciles text without retaining obsolete interaction color.
#[wasm_bindgen]
pub async fn retained_header_changed_caption_restores_plain_color() {
    retained_header_transition("replacement caption").await;
}

async fn retained_header_transition(plain_text: &str) {
    use crate::markdown_support::{js, publish};
    let mut surface = surface();
    let pack = surface.pack.clone();
    let presentation = pack.presentation(maestro_theme::BrandMode::Dark).unwrap();
    let collapsible = recipe(
        ToolHeaderState::Complete,
        false,
        Some(Rc::new(Cell::new(false))),
        "caption",
    );
    let plain = recipe(ToolHeaderState::Complete, false, None, plain_text);
    let mut row = ToolRenderState::default();
    browser::draw(&mut surface, &plain, &mut row);
    let response = browser::draw(&mut surface, &collapsible, &mut row);
    surface.events = vec![egui::Event::PointerMoved(response.rect.center())];
    browser::draw(&mut surface, &collapsible, &mut row);
    browser::draw(&mut surface, &collapsible, &mut row);
    let _ = js("globalThis.retainedCaption=document.querySelector('.maestro-tool-caption')");
    publish("hoverText", &presentation.colors["text"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    surface
        .context
        .memory_mut(|memory| memory.request_focus(response.id));
    browser::draw(&mut surface, &collapsible, &mut row);
    surface.events = vec![egui::Event::PointerGone];
    let response = browser::draw(&mut surface, &plain, &mut row);
    publish("plainText", &plain_text.into());
    check(
        "document.querySelector('.maestro-tool-caption') === retainedCaption && retainedCaption.textContent === plainText",
    );
    publish("expectedColor", &presentation.colors["muted"].into());
    check(
        "(()=>{const control=document.createElement('span');control.style.color=expectedColor;document.body.appendChild(control);const matches=getComputedStyle(retainedCaption).color===getComputedStyle(control).color;control.remove();return matches})()",
    );
    publish("hoverText", &presentation.colors["muted"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    assert_eq!(surface.cursor, egui::CursorIcon::Default);
    assert!(!response.has_focus());
    let icons = browser::glyphs(&surface);
    let border = egui::Rect::from_min_max(
        egui::pos2(icons[0].right() + 2., response.rect.top()),
        egui::pos2(response.rect.right() - 2., response.rect.top() + 2.),
    );
    assert!(
        browser::pixels(&surface, border)
            .iter()
            .all(|pixel| pixel[3] == 0)
    );
    surface.events = vec![egui::Event::PointerMoved(response.rect.center())];
    browser::draw(&mut surface, &collapsible, &mut row);
    browser::draw(&mut surface, &collapsible, &mut row);
    publish("hoverText", &presentation.colors["text"].into());
    crate::support::reading_paint::observe("tool-header-hover").await;
    assert_eq!(surface.cursor, egui::CursorIcon::PointingHand);
}
