//! Native usage callbacks remain current even outside the canvas-visible range.
use super::aggregate::{Language, draw};
use crate::reading_slots_support::{Slots, js};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Changed and removed actions reach retained offscreen native usage without a session event.
/// # Panics
/// Panics if an offscreen control invokes an obsolete callback or its root is replaced.
#[wasm_bindgen]
pub fn transcript_offscreen_cost_actions_remain_current() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    let messages = (0..30)
        .map(|_| {
            let mut message = maestro_app::faux_assistant_message(
                "usage row",
                maestro_app::FauxAssistantMessageOptions::default(),
            );
            message.usage.input = 3.;
            AgentMessage::Assistant(Arc::new(RwLock::new(message)))
        })
        .collect();
    agent.state().write().unwrap().set_messages(messages);
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(agent)).unwrap();
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let hit = first.clone();
    interface.on_cost_click = Some(Rc::new(move || hit.set(hit.get() + 1)));
    draw(&scene, &mut interface, &i18n);
    let control = js(
        &JsValue::NULL,
        "return document.querySelectorAll('.maestro-message-usage')[29]",
    );
    assert_eq!(js(&control, "const v=value.closest('.maestro-transcript').parentElement;return value.isConnected && value.getBoundingClientRect().top>v.getBoundingClientRect().bottom && v.scrollTop===0").as_bool(), Some(true));
    js(&control, "value.click()");
    assert_eq!(first.get(), 1);
    let hit = second.clone();
    interface.on_cost_click = Some(Rc::new(move || hit.set(hit.get() + 1)));
    draw(&scene, &mut interface, &i18n);
    js(&control, "value.click()");
    assert_eq!([first.get(), second.get()], [1, 1]);
    interface.on_cost_click = None;
    draw(&scene, &mut interface, &i18n);
    js(&control, "value.click()");
    assert_eq!([first.get(), second.get()], [1, 1]);
    assert_eq!(js(&control, "return value===document.querySelectorAll('.maestro-message-usage')[29] && value.isConnected").as_bool(), Some(true));
}
