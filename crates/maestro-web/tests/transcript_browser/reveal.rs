//! Native browser Find, keyboard focus and clipped pointer admission on the actual transcript.
use super::aggregate::{Language, Renderer, draw, transcript_root};
use super::{check, js};
use crate::reading_slots_support::Slots;
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{Agent, AgentMessage, NoCustomMessages};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Native Find and keyboard navigation reveal offscreen text before its first returned canvas frame.
/// # Panics
/// Panics if browser navigation fails, canvas culling is stale or a clipped link accepts a pointer.
#[wasm_bindgen]
pub async fn transcript_scroll_reveals_found_or_focused_content() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
    let session = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    session.state().write().unwrap().set_messages(
        (0..40)
            .map(|n| {
                AgentMessage::User(Arc::new(RwLock::new(maestro_app::UserMessage {
                    content: maestro_app::UserContent::Text(format!(
                        "[transcript-reveal-needle-{n}](https://example.com/{n})"
                    )),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(session)).unwrap();
    draw(&scene, &mut interface, &i18n);
    let root = transcript_root();
    calls.borrow_mut().clear();
    navigate(&root, "find").await;
    revealed(&root);
    draw(&scene, &mut interface, &i18n);
    assert!(calls.take().iter().any(|text| text.contains("needle-39]")));
    check(
        &root,
        "const link=value.querySelectorAll('a')[0],r=link.getBoundingClientRect();return !link.contains(document.elementFromPoint(r.x+r.width/2,r.y+r.height/2))",
    );
    js(
        &root,
        "globalThis.clippedClicks=0;value.querySelectorAll('a')[0].addEventListener('click',()=>clippedClicks++)",
    );
    navigate(&root, "clipped").await;
    check(&root, "return globalThis.clippedClicks===0");
    js(
        &root,
        "value.parentElement.scrollTop=0;value.querySelectorAll('a')[38].focus({preventScroll:true})",
    );
    calls.borrow_mut().clear();
    navigate(&root, "focus").await;
    revealed(&root);
    check(
        &root,
        "return document.activeElement===value.querySelectorAll('a')[39]",
    );
    draw(&scene, &mut interface, &i18n);
    assert!(calls.take().iter().any(|text| text.contains("needle-39]")));
}

/// The existing browser witness route drives actual OS Find and keyboard input.
async fn navigate(root: &JsValue, operation: &str) {
    let promise = js(
        root,
        &format!("return globalThis.transcriptNavigation('{operation}')"),
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
}

/// Observe real viewport containment, not a requested scroll destination.
fn revealed(root: &JsValue) {
    check(
        root,
        "const row=value.querySelectorAll('a')[39].getBoundingClientRect(),v=value.parentElement.getBoundingClientRect();return value.parentElement.scrollTop>0 && row.top<v.bottom && row.bottom>v.top",
    );
}
