//! Measured native empty-session and bounded transcript layout.
use super::aggregate::{Language, draw};
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// The unbound literal remains native; binding replaces it with the centered padded transcript.
/// # Panics
/// Panics if literal text, native bounds, padding or retirement differs.
#[wasm_bindgen]
pub fn transcript_missing_session_and_viewport_match_layout_and_literal_text() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let mut interface = AgentInterface::new(scene.host.clone());
    draw(&scene, &mut interface, &i18n);
    let empty = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript-empty')",
    );
    check(
        &empty,
        "if(!value)return false;const s=getComputedStyle(value),r=value.getBoundingClientRect();return value.innerText==='No session set' && s.textAlign==='center' && s.paddingTop==='16px' && s.paddingLeft==='16px' && r.width>700 && !document.body.innerText.includes('No session available')",
    );
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![AgentMessage::User(Arc::new(RwLock::new(
            maestro_app::UserMessage {
                content: maestro_app::UserContent::Text("bounded native row".into()),
                timestamp: 0.,
            },
        )))]);
    interface.set_session(Some(agent)).unwrap();
    draw(&scene, &mut interface, &i18n);
    check(&empty, "return !value.isConnected");
    let root = super::aggregate::transcript_root();
    check(
        &root,
        "const r=value.getBoundingClientRect(),p=value.parentElement.getBoundingClientRect(),s=getComputedStyle(value);return Math.abs(r.width-768)<0.1 && Math.abs((r.left-p.left)-(p.right-r.right))<1 && s.paddingTop==='16px' && s.paddingLeft==='16px' && s.paddingRight==='16px' && s.paddingBottom==='0px'",
    );
    interface.set_session(None).unwrap();
    draw(&scene, &mut interface, &i18n);
    check(
        &JsValue::NULL,
        "return document.querySelectorAll('.maestro-transcript-empty').length===1 && !document.querySelector('.maestro-transcript')",
    );
}

/// A non-default document rem keeps native slots aligned with the painted copy glyph.
/// # Panics
/// Panics if measured native padding and actual canvas glyph coverage diverge.
#[wasm_bindgen]
pub fn transcript_resolved_rem_aligns_native_slots_and_canvas_paint() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    maestro_web::register_message_renderer(
        "user",
        Rc::new(super::aggregate::Renderer(Rc::new(
            std::cell::RefCell::new(vec![]),
        ))),
    );
    let old = js(
        &JsValue::NULL,
        "const previous=document.documentElement.style.fontSize;document.documentElement.style.fontSize='20px';return previous",
    );
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![AgentMessage::User(Arc::new(RwLock::new(
            maestro_app::UserMessage {
                content: maestro_app::UserContent::Text(
                    "```text\nresolved document rem\n```".into(),
                ),
                timestamp: 0.,
            },
        )))]);
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(agent)).unwrap();
    let mut output = super::aggregate::draw_frame(
        &scene,
        &mut interface,
        &i18n,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 180.)),
        vec![],
    )
    .unwrap();
    let mut paint = crate::canvas_paint::Paint::default();
    paint.update(&output);
    output.textures_delta.clear();
    let values = js(
        &JsValue::NULL,
        "const r=document.querySelector('.maestro-transcript .control-slot').getBoundingClientRect();return [r.x+r.width/2,r.top,r.bottom,document.querySelector('.maestro-transcript-slot').getBoundingClientRect().left]",
    );
    let values = js_sys::Float32Array::new(&values);
    let center = values.get_index(0);
    let painted_center = copy_glyph_center(&scene, &paint, &output.shapes);
    js(&old, "document.documentElement.style.fontSize=value");
    assert!(
        (painted_center - center).abs() < 1.,
        "painted center {painted_center} != native center {center}"
    );
    assert!(
        (values.get_index(3) - 20.).abs() < 1.,
        "native padding resolves to one document rem"
    );
}

/// Measure final clipped glyph coverage, not its requested placement.
fn copy_glyph_center(
    scene: &Slots,
    paint: &crate::canvas_paint::Paint,
    shapes: &[egui::epaint::ClippedShape],
) -> f32 {
    let xs: Vec<_> = (650..800)
        .filter(|x| {
            (20..65).any(|y| {
                paint.pixel(
                    &scene.context,
                    shapes,
                    egui::pos2(
                        f32::from(u16::try_from(*x).unwrap()) + 0.5,
                        f32::from(u16::try_from(y).unwrap()) + 0.5,
                    ),
                )[3] > 0.
            })
        })
        .collect();
    assert!(!xs.is_empty(), "the copy glyph must paint");
    f32::from(u16::try_from(xs[0] + xs[xs.len() - 1] + 1).unwrap()) / 2.
}
