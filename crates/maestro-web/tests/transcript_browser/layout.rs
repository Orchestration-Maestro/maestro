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
