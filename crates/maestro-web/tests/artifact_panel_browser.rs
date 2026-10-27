//! Real browser retained-panel and existing native-registry qualification.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::presentation;
use maestro_theme::BrandMode;
use maestro_web::browser::attachment_content::AttachmentHost;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::panel_browser_support;
use panel_browser_support::Panel;
/// Execute observation-only native DOM operations in the runner's actual document.
fn js(code: &str) -> JsValue {
    js_sys::eval(code).unwrap()
}
/// Assert a native browser predicate without reconstructing its result in Rust.
fn check(code: &str) {
    assert!(js(code).as_bool().unwrap(), "browser predicate: {code}");
}
/// Leaving and returning to panel overlay preserves the PDF's native root and scroll.
#[wasm_bindgen]
pub fn artifact_panel_browser_pdf_overlay_return_keeps_root_and_scroll() {
    use base64::Engine as _;
    let view = Panel::new();
    let content = base64::engine::general_purpose::STANDARD.encode(include_bytes!(
        "../../maestro-app/tests/fixtures/pdf/synthetic-multipage.pdf"
    ));
    presentation::ready((view.caller.tool().execute)(
        "call".into(),
        serde_json::json!({"command":"create","filename":"a.pdf","content":content}),
        None,
        None,
    ))
    .unwrap();
    view.panel.set_overlay(true);
    for _ in 0..5 {
        view.draw(vec![]);
    }
    js(
        "globalThis.pdfRoot=document.querySelector('.maestro-pdf');globalThis.pdfWrapper=pdfRoot.parentElement;pdfWrapper.scrollTop=41.5;globalThis.pdfScroll=pdfWrapper.scrollTop;",
    );
    check("pdfRoot.querySelectorAll('img').length===3 && pdfScroll>0");
    view.panel.set_overlay(false);
    view.draw(vec![]);
    view.panel.set_overlay(true);
    view.draw(vec![]);
    check(
        "pdfRoot===document.querySelector('.maestro-pdf') && pdfRoot.isConnected && pdfWrapper.scrollTop===pdfScroll",
    );
    drop(view);
    check("!pdfRoot.isConnected");
}

/// Two generic views preserve native roots, text selection and fractional scrolling.
#[wasm_bindgen]
pub fn artifact_panel_browser_switches_keep_native_roots() {
    let view = Panel::new();
    view.create("a.md.zip", "AAEC");
    presentation::ready((view.caller.tool().execute)(
        "call".into(),
        serde_json::json!({"command":"rewrite","filename":"a.md.zip","content":"AAEC//79"}),
        None,
        None,
    ))
    .unwrap();
    view.create("b.zip", "AAEC");
    js(
        "globalThis.roots=[...document.querySelectorAll('.maestro-generic-artifact')];globalThis.a=roots[0];globalThis.b=roots[1];globalThis.wa=a.parentElement;globalThis.wb=b.parentElement;a.style.minHeight='900px';b.style.minHeight='900px';wa.scrollTop=23.5;wb.scrollTop=41.5;globalThis.sa=wa.scrollTop;globalThis.sb=wb.scrollTop;globalThis.selected=a.querySelector('p span');const range=document.createRange();range.selectNodeContents(selected);getSelection().removeAllRanges();getSelection().addRange(range);",
    );
    check("sa>0 && sb>0 && sa%1!==0 && sb%1!==0");
    for name in ["a.md.zip", "b.zip", "a.md.zip"] {
        view.caller.show_artifact(name);
        view.draw(vec![]);
    }
    check(
        "document.querySelectorAll('.maestro-generic-artifact').length===2 && a===roots[0] && b===roots[1] && getComputedStyle(wa).visibility==='visible' && getComputedStyle(wb).visibility==='hidden' && wa.scrollTop===sa && wb.scrollTop===sb && getSelection().toString()===selected.textContent",
    );
    let output = view.draw(vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Mesh(mesh) => Some(mesh.calc_bounds().center()),
            _ => None,
        })
        .unwrap();
    view.draw(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    drop(view);
    check("!a.isConnected && !b.isConnected");
}

/// Independent hide survives ordinary publication, reading publication and modal changes.
#[wasm_bindgen]
pub fn artifact_panel_browser_hidden_state_survives_all_writers() {
    let view = Panel::new();
    let id = egui::Id::new("retained-ordinary");
    view.host.set_retained_visibility(id, false, None).unwrap();
    ordinary(&view, id);
    js("globalThis.ordinary=document.querySelector('pre').parentElement;");
    check(
        "getComputedStyle(ordinary).visibility==='hidden' && getComputedStyle(ordinary).pointerEvents==='none'",
    );
    ordinary(&view, id);
    let modal = egui::Id::new("neighbor-modal");
    view.host.set_overlay(modal, true).unwrap();
    view.host.set_overlay(modal, false).unwrap();
    check("getComputedStyle(ordinary).visibility==='hidden'");
    view.host.set_retained_visibility(id, true, None).unwrap();
    check("getComputedStyle(ordinary).visibility==='visible'");
    view.create("a.zip", "AAEC");
    js(
        "globalThis.a=document.querySelector('.maestro-generic-artifact');globalThis.wa=a.parentElement;",
    );
    for overlay in [true, false] {
        view.panel.set_overlay(overlay);
        view.panel.set_collapsed(true);
        view.draw(vec![]);
        view.host.set_overlay(modal, true).unwrap();
        view.host.set_overlay(modal, false).unwrap();
        check(
            "a.isConnected && getComputedStyle(wa).visibility==='hidden' && getComputedStyle(wa).pointerEvents==='none'",
        );
        view.panel.set_collapsed(false);
        view.draw(vec![]);
        check(
            "a===document.querySelector('.maestro-generic-artifact') && getComputedStyle(wa).visibility==='visible'",
        );
    }
    view.host.remove(id);
    check("!ordinary.isConnected");
}

/// Exercise ordinary publication in the same host registry used by reading roots.
fn ordinary(view: &Panel, id: egui::Id) {
    use maestro_web::browser::attachment_content::{
        AttachmentContent, AttachmentSurface, TextView,
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200., 100.));
    view.host
        .present(
            AttachmentSurface {
                id,
                rect,
                clip_rect: rect,
                pixels_per_point: 1.,
                opacity: 1.,
                occluders: vec![],
                content: AttachmentContent::Text {
                    text: "unrelated",
                    view: TextView::Plain,
                },
            },
            &view.pack.presentation(BrandMode::Dark).unwrap(),
        )
        .unwrap();
}

/// Candidate publication inherits the owning entry's latest policy and rolls back failures.
#[wasm_bindgen]
pub fn artifact_panel_browser_pending_replacement_inherits_visibility() {
    use maestro_web::browser::attachment_content::ReadingPhase::{Measure, Publish};
    let view = Panel::new();
    let id = egui::Id::new("replacement");
    let first = reading_content("old-root");
    view.host.set_retained_visibility(id, false, None).unwrap();
    reading(&view, id, &first, Measure, None).unwrap();
    reading(&view, id, &first, Publish, None).unwrap();
    js(
        "globalThis.old=document.querySelector('.old-root');globalThis.oldWrapper=old.parentElement;",
    );
    check("getComputedStyle(oldWrapper).visibility==='hidden'");
    let second = reading_content("new-root");
    reading(&view, id, &second, Measure, None).unwrap();
    let modal = egui::Id::new("new-modal");
    view.host
        .set_retained_visibility(id, true, Some(modal))
        .unwrap();
    view.host.set_overlay(modal, true).unwrap();
    reading(&view, id, &second, Publish, None).unwrap();
    check(
        "!old.isConnected && getComputedStyle(document.querySelector('.new-root').parentElement).visibility==='visible'",
    );
    view.host.set_retained_visibility(id, false, None).unwrap();
    view.host.set_overlay(modal, false).unwrap();
    let third = reading_content("retry-root");
    reading(&view, id, &third, Measure, None).unwrap();
    js(
        "globalThis.surviving=document.querySelector('.new-root');globalThis.failed=document.querySelector('.retry-root');const wrapper=failed.parentElement;const original=wrapper.setAttribute;wrapper.setAttribute=function(...args){this.setAttribute=original;throw new Error('publish failure');};",
    );
    assert!(reading(&view, id, &third, Publish, None).is_err());
    check(
        "surviving.isConnected && !failed.isConnected && getComputedStyle(surviving.parentElement).visibility==='hidden'",
    );
    reading(&view, id, &third, Measure, None).unwrap();
    reading(&view, id, &third, Publish, None).unwrap();
    check(
        "!surviving.isConnected && getComputedStyle(document.querySelector('.retry-root').parentElement).visibility==='hidden'",
    );
    view.host.set_retained_visibility(id, true, None).unwrap();
    check(
        "getComputedStyle(document.querySelector('.retry-root').parentElement).visibility==='visible'",
    );
    view.host.remove(id);
    check("document.querySelector('.retry-root')===null");
}

/// A caller-owned actual native root factory for registry transaction inputs.
fn reading_content(
    class: &'static str,
) -> Rc<maestro_web::browser::attachment_content::ReadingContent> {
    use maestro_web::browser::attachment_content::{
        AttachmentHostError, ReadingContent, ReadingRoot,
    };
    Rc::new(ReadingContent {
        mount: Rc::new(move |document| {
            let element = document
                .create_element("section")
                .map_err(|_| AttachmentHostError::Host("create failed".into()))?;
            element.set_class_name(class);
            element.set_text_content(Some("retained native text"));
            Ok(ReadingRoot {
                frame_style: "",
                decoration: None,
                commit: None,
                owner: None,
                element,
                slots: vec![],
            })
        }),
    })
}

/// Run the existing reading protocol without a second publication implementation.
fn reading(
    view: &Panel,
    id: egui::Id,
    content: &Rc<maestro_web::browser::attachment_content::ReadingContent>,
    phase: maestro_web::browser::attachment_content::ReadingPhase,
    modal: Option<egui::Id>,
) -> Result<(), maestro_web::browser::attachment_content::AttachmentHostError> {
    use maestro_web::browser::attachment_content::{
        AttachmentContent, AttachmentSurface, ReadingPlacement,
    };
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(200., 100.));
    view.host.present(
        AttachmentSurface {
            id,
            rect,
            clip_rect: rect,
            pixels_per_point: 1.,
            opacity: 1.,
            occluders: vec![],
            content: AttachmentContent::Reading(ReadingPlacement {
                intrinsic_width: false,
                content: content.clone(),
                phase,
                sizes: &[],
                modal,
                context: &view.context,
            }),
        },
        &view.pack.presentation(BrandMode::Dark).unwrap(),
    )
}

/// Disconnect hides input without detaching roots or losing native selection and scroll.
#[wasm_bindgen]
pub fn artifact_panel_browser_disconnect_keeps_selection_and_scroll() {
    let view = Panel::new();
    view.create("a.zip", "AAEC");
    view.create("b.zip", "AAEC");
    view.caller.show_artifact("a.zip");
    view.draw(vec![]);
    js(
        "globalThis.a=document.querySelectorAll('.maestro-generic-artifact')[0];globalThis.b=document.querySelectorAll('.maestro-generic-artifact')[1];globalThis.wa=a.parentElement;a.style.minHeight='900px';wa.scrollTop=23.5;globalThis.scroll=wa.scrollTop;globalThis.selected=a.querySelector('p span');const range=document.createRange();range.selectNodeContents(selected);getSelection().removeAllRanges();getSelection().addRange(range);",
    );
    view.panel.disconnected_callback().unwrap();
    view.draw(vec![]);
    assert_eq!(view.caller.active_filename().as_deref(), Some("a.zip"));
    check(
        "a.isConnected && b.isConnected && getComputedStyle(wa).visibility==='hidden' && getComputedStyle(wa).pointerEvents==='none'",
    );
    view.panel
        .get_or_create_artifact_element("b.zip", "Yg==")
        .unwrap();
    view.panel.connected_callback(&view.caller).unwrap();
    view.draw(vec![]);
    check(
        "a===document.querySelectorAll('.maestro-generic-artifact')[0] && b===document.querySelectorAll('.maestro-generic-artifact')[1] && document.querySelectorAll('.maestro-generic-artifact').length===2 && getComputedStyle(wa).visibility==='visible' && wa.scrollTop===scroll && getSelection().toString()===selected.textContent",
    );
    presentation::ready((view.caller.tool().execute)(
        "call".into(),
        serde_json::json!({"command":"delete","filename":"a.zip"}),
        None,
        None,
    ))
    .unwrap();
    view.draw(vec![]);
    check(
        "!a.isConnected && b.isConnected && getComputedStyle(b.parentElement).visibility==='visible'",
    );
    drop(view);
    check("!b.isConnected");
}

/// Ordinary modal children release on close while panel-retained roots survive.
#[wasm_bindgen]
pub fn artifact_panel_browser_modal_children_release_while_panel_roots_survive() {
    use maestro_web::browser::attachment_content::ReadingPhase::{Measure, Publish};
    let view = Panel::new();
    view.create("a.zip", "AAEC");
    view.panel.set_overlay(true);
    view.draw(vec![]);
    js("globalThis.a=document.querySelector('.maestro-generic-artifact');");
    let unrelated = egui::Id::new("unrelated");
    ordinary(&view, unrelated);
    js("globalThis.unrelated=document.querySelector('pre').parentElement;");
    // Ordinary modal-child ownership stays distinct from a retained panel policy.
    let nested = egui::Id::new("nested-ordinary-modal");
    let child = egui::Id::new("nested-child");
    let content = reading_content("modal-child");
    view.host.set_overlay(nested, true).unwrap();
    reading(&view, child, &content, Measure, Some(nested)).unwrap();
    reading(&view, child, &content, Publish, Some(nested)).unwrap();
    check("document.querySelector('.modal-child').isConnected");
    view.host.set_overlay(nested, false).unwrap();
    check(
        "document.querySelector('.modal-child')===null && a.isConnected && unrelated.isConnected",
    );
    view.panel.disconnected_callback().unwrap();
    view.panel.connected_callback(&view.caller).unwrap();
    view.draw(vec![]);
    check("a===document.querySelector('.maestro-generic-artifact')");
    view.host.remove(unrelated);
    drop(view);
    check("!a.isConnected && !unrelated.isConnected");
}

/// Opaque native content cannot cover either canvas shell divider.
#[wasm_bindgen]
pub fn artifact_panel_browser_body_leaves_shell_dividers_visible() {
    let view = Panel::new();
    view.create("a.zip", "AAEC");
    for overlay in [false, true] {
        view.panel.set_overlay(overlay);
        view.draw(vec![]);
        let output = view.draw(vec![]);
        let document = web_sys::window().unwrap().document().unwrap();
        let root = document
            .query_selector(".maestro-generic-artifact")
            .unwrap()
            .unwrap();
        let rect = root.parent_element().unwrap().get_bounding_client_rect();
        let boundaries: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::LineSegment { points, stroke }
                    if stroke.width.to_bits() == 1_f32.to_bits() =>
                {
                    Some(*points)
                }
                _ => None,
            })
            .collect();
        assert_eq!(boundaries.len(), if overlay { 1 } else { 2 });
        for line in boundaries {
            let point = line[0].lerp(line[1], 0.5);
            // Native rectangles share the canvas's origin in this runner.
            let hit = document.element_from_point(point.x, point.y).unwrap();
            assert_eq!(
                hit.tag_name(),
                "CANVAS",
                "divider covered by native body at {point:?}; body {rect:?}"
            );
        }
        assert!(rect.width() > 0. && rect.height() > 0.);
        let body = document
            .element_from_point(
                num_traits::cast(rect.x() + 10.).unwrap(),
                num_traits::cast(rect.y() + 10.).unwrap(),
            )
            .unwrap();
        assert_ne!(body.tag_name(), "CANVAS");
    }
}
