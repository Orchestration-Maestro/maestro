//! Retained native placement and presentation follow external inputs.
use super::aggregate::{Language, Renderer, draw, transcript_root};
use crate::reading_slots_support::{Slots, js};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Canvas translation and a presentation switch reconcile without a session event.
/// # Panics
/// Panics if native rows keep their previous origin or presentation.
#[wasm_bindgen]
pub fn transcript_native_rows_follow_canvas_origin_and_presentation() {
    let mut scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    maestro_web::register_message_renderer(
        "user",
        Rc::new(Renderer(Rc::new(RefCell::new(vec![])))),
    );
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    agent.state().write().unwrap().set_messages(
        (0..30)
            .map(|_| {
                AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text("retained native text".into()),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(agent)).unwrap();
    draw(&scene, &mut interface, &i18n);
    let root = transcript_root();
    let before = js(
        &root,
        "const canvas=document.getElementById('canvas'),frame=value.parentElement.parentElement;const r=frame.getBoundingClientRect(),c=canvas.getBoundingClientRect();return {root:value,last:value.lastElementChild.firstElementChild,left:r.left-c.left,top:r.top-c.top,color:getComputedStyle(value.lastElementChild.querySelector('p')).color}",
    );
    js(
        &JsValue::NULL,
        "document.getElementById('canvas').style.transform='translate(40px,30px)'",
    );
    draw(&scene, &mut interface, &i18n);
    let aligned = js(
        &before,
        "const r=value.root.parentElement.parentElement.getBoundingClientRect(),c=document.getElementById('canvas').getBoundingClientRect();return Math.abs(r.left-c.left-value.left)<1 && Math.abs(r.top-c.top-value.top)<1",
    );
    scene.mode = maestro_theme::BrandMode::Light;
    draw(&scene, &mut interface, &i18n);
    let changed = js(
        &before,
        "return value.root.isConnected && value.last===value.root.lastElementChild.firstElementChild && getComputedStyle(value.root.lastElementChild.querySelector('p')).color!==value.color",
    );
    js(
        &JsValue::NULL,
        "document.getElementById('canvas').style.transform=''",
    );
    assert_eq!(
        aligned.as_bool(),
        Some(true),
        "native placement must follow origin-only canvas movement"
    );
    assert_eq!(
        changed.as_bool(),
        Some(true),
        "offscreen native rows must take the new presentation"
    );
}
