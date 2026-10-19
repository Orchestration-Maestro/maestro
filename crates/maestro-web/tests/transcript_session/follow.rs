//! Follow alignment on real session-event appends and streaming publication.
use super::{Model, frame};
use crate::{
    canvas_paint::Paint,
    reading_slots_support::{Slots, js},
    transcript_browser::{
        aggregate::{Language, Renderer, draw_frame},
        pointer,
        transaction::assert_follow_frame,
    },
};
use maestro_app::{AgentEvent, AgentMessage, UserContent, UserMessage};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Real event growth paints and clicks at the new bottom before another canvas frame.
/// # Panics
/// Panics if an event append or streaming update returns stale paint or action geometry.
#[wasm_bindgen]
pub async fn transcript_follow_commits_event_and_stream_growth_before_paint() {
    let model = Model::new();
    let agent = model.session();
    agent.state().write().unwrap().set_messages(
        (0..30)
            .map(|n| {
                AgentMessage::User(Arc::new(RwLock::new(UserMessage {
                    content: UserContent::Text(format!("```text\nevent history {n}\n```")),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
    let scene = Rc::new(Slots::new());
    let language = Rc::new(I18n::new(Rc::new(Language)).unwrap());
    let calls = Rc::new(RefCell::new(vec![]));
    maestro_web::register_message_renderer("user", Rc::new(Renderer(calls.clone())));
    let view = Rc::new(RefCell::new(AgentInterface::new(scene.host.clone())));
    view.borrow_mut().set_session(Some(agent.clone())).unwrap();
    let paint = Rc::new(RefCell::new(Paint::default()));
    aligned(
        &scene,
        &mut view.borrow_mut(),
        &language,
        &mut paint.borrow_mut(),
    );
    let observed = Rc::new(RefCell::new(Vec::new()));
    let retained = observed.clone();
    let subscription = agent.subscribe(Rc::new(move |event, _| {
            let event = event.read().unwrap();
            let user = matches!(&*event, AgentEvent::MessageEnd {message} if matches!(message, AgentMessage::User(_)));
            let update = matches!(&*event, AgentEvent::MessageUpdate {..});
            drop(event);
            calls.borrow_mut().clear();
            aligned(&scene, &mut view.borrow_mut(), &language, &mut paint.borrow_mut());
            let count = calls.borrow().len();
            let unique = calls.borrow().iter().cloned().collect::<std::collections::HashSet<_>>().len();
            assert_eq!(count, unique, "event frame invokes each stable recipe only once");
            let (scene, language, view, paint, observed) = (scene.clone(), language.clone(), view.clone(), paint.clone(), retained.clone());
            Box::pin(async move {
                if user {
                    click_appended(&scene, &view, &language).await;
                    observed.borrow_mut().push("append click");
                }
                if update {
                    frame().await;
                    aligned(&scene, &mut view.borrow_mut(), &language, &mut paint.borrow_mut());
                    observed.borrow_mut().push("stream paint");
                }
                Ok(())
            })
    }));
    model.prompt(&agent, "```text\nevent growth\n```").await;
    subscription.unsubscribe();
    assert!(observed.borrow().contains(&"append click"));
    assert!(observed.borrow().contains(&"stream paint"));
}
/// Compare returned clipped glyph paint with native geometry at each event.
fn aligned(scene: &Slots, view: &mut AgentInterface, language: &I18n, paint: &mut Paint) {
    let mut output = draw_frame(
        scene,
        view,
        language,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 180.)),
        vec![],
    )
    .unwrap();
    assert_follow_frame(scene, &mut output, paint);
}

/// Deliver the first gesture after the appended code control has appeared.
async fn click_appended(scene: &Slots, view: &RefCell<AgentInterface>, language: &I18n) {
    let saved = pointer::capture();
    js(
        &JsValue::NULL,
        "const r=[...document.querySelectorAll('.maestro-transcript .control-slot')].at(-1).getBoundingClientRect();globalThis.transcriptTarget=[r.x+r.width/2,r.y+r.height/2];globalThis.transcriptHit=document.elementFromPoint(...transcriptTarget)===document.getElementById('canvas')",
    );
    assert_eq!(
        js(&JsValue::NULL, "return transcriptHit").as_bool(),
        Some(true)
    );
    pointer::click().await;
    pointer::deliver_pointer(scene, &mut view.borrow_mut(), language);
    assert_eq!(
        js(&JsValue::NULL, "return transcriptCopies.join('|')")
            .as_string()
            .as_deref(),
        Some("event growth\n")
    );
    pointer::release(&saved);
}
