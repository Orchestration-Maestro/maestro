//! Native viewport growth under the public reader-follow override.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::reading_slots_support::{Slots, js};
use crate::transcript_browser::aggregate::{Language, draw};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Append through the actual Agent history owner and admit a containing-view refresh.
fn grow(
    session: &Rc<Agent<BrowserMessages<NoCustomMessages>>>,
    interface: &mut AgentInterface,
    count: usize,
) {
    let state = session.state().read().unwrap();
    let mut messages = state.messages().read().unwrap().clone();
    drop(state);
    let start = messages.len();
    messages.extend((start..start + count).map(|n| {
        AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
            content: maestro_app::UserContent::Text(format!("growing row {n}")),
            timestamp: 0.,
        })))
    }));
    session.state().write().unwrap().set_messages(messages);
    interface.set_session(Some(session.clone())).unwrap();
}
/// Await native callback delivery rather than a timer or a guessed delay.
async fn frame() {
    let promise = js(
        &JsValue::NULL,
        "return new Promise(resolve=>requestAnimationFrame(resolve))",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}
/// False preserves the reader's actual position; true follows actual content height in both directions.
/// # Panics
/// Panics if overrides are ignored before connection or later growth.
#[wasm_bindgen]
pub async fn follow_override_controls_later_content_growth() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let session = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    grow(&session, &mut interface, 20);
    draw(&scene, &mut interface, &i18n);
    let root = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')",
    );
    assert_eq!(
        js(&root, "return value.parentElement.scrollTop").as_f64(),
        Some(0.)
    );
    grow(&session, &mut interface, 5);
    draw(&scene, &mut interface, &i18n);
    assert_eq!(
        js(&root, "return value.parentElement.scrollTop").as_f64(),
        Some(0.)
    );
    interface.set_auto_scroll(true);
    grow(&session, &mut interface, 5);
    draw(&scene, &mut interface, &i18n);
    assert_eq!(js(&root,"const v=value.parentElement;const observed=v.scrollTop;v.scrollTop=v.scrollHeight;return observed===v.scrollTop && observed>0").as_bool(),Some(true));
    frame().await;
    draw(&scene, &mut interface, &i18n);
    interface.set_auto_scroll(false);
    js(
        &root,
        "value.parentElement.scrollTop=30.5;value.parentElement.dispatchEvent(new Event('scroll'))",
    );
    draw(&scene, &mut interface, &i18n);
    let retained = js(&root, "return value.parentElement.scrollTop")
        .as_f64()
        .unwrap();
    grow(&session, &mut interface, 5);
    draw(&scene, &mut interface, &i18n);
    assert_eq!(
        js(&root, "return value.parentElement.scrollTop")
            .as_f64()
            .unwrap()
            .to_bits(),
        retained.to_bits()
    );
    interface.set_auto_scroll(true);
    grow(&session, &mut interface, 5);
    draw(&scene, &mut interface, &i18n);
    assert_eq!(js(&root,"const v=value.parentElement;const observed=v.scrollTop;v.scrollTop=v.scrollHeight;return observed===v.scrollTop").as_bool(),Some(true));
}
