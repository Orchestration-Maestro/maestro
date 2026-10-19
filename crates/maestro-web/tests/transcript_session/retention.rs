//! Retained stable native owners across real streaming turns and scaled placement.
use super::{Model, frame};
use crate::{
    reading_slots_support::{Slots, js},
    transcript_browser::aggregate::{Language, Renderer, draw_in_rect, transcript_root},
};
use maestro_app::{AgentEvent, AgentMessage, UserContent, UserMessage};
use maestro_web::{AgentInterface, I18n};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::prelude::*;

/// Keep a fractional canvas origin under independent horizontal and vertical browser scales.
fn rect(height: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(12.5, 18.25), egui::vec2(800., height))
}
/// Native stable-row identities, exact selection endpoints and reader-away offset.
fn save() -> JsValue {
    js(
        &transcript_root(),
        "const slots=[...value.querySelectorAll('.maestro-transcript-slot')].slice(0,30);const range=document.createRange();range.selectNodeContents(slots[0].querySelector('p'));getSelection().removeAllRanges();getSelection().addRange(range);const v=value.parentElement;v.scrollTop=31.5;v.dispatchEvent(new Event('scroll'));return {root:value,slots,children:slots.map(n=>n.firstElementChild),anchor:getSelection().anchorNode,anchorOffset:getSelection().anchorOffset,focus:getSelection().focusNode,focusOffset:getSelection().focusOffset,top:v.scrollTop}",
    )
}
/// Repeated publication must preserve native owners, not merely equal visible strings.
fn retained(saved: &JsValue) {
    assert!(js(saved, "const slots=[...value.root.querySelectorAll('.maestro-transcript-slot')];const s=getSelection();return value.root.isConnected && value.slots.every((node,i)=>node===slots[i] && node.firstElementChild===value.children[i] && value.children[i].isConnected) && s.anchorNode===value.anchor && s.anchorOffset===value.anchorOffset && s.focusNode===value.focus && s.focusOffset===value.focusOffset && value.root.parentElement.scrollTop===value.top").as_bool().unwrap());
}
/// Observe actual containing and child positions on both axes, not a declared transform string.
fn positioned() {
    let observation = js(
        &transcript_root(),
        "const c=document.getElementById('canvas');return JSON.stringify({canvas:c.getBoundingClientRect().toJSON(),frame:value.parentElement.parentElement.getBoundingClientRect().toJSON(),slots:[...value.querySelectorAll('.maestro-transcript-slot')].map(s=>({slot:s.getBoundingClientRect().toJSON(),child:s.firstElementChild?.getBoundingClientRect().toJSON()})).filter(s=>s.child && (Math.abs(s.child.left-s.slot.left)>=0.2 || Math.abs(s.child.top-s.slot.top)>=0.2))})",
    );
    assert!(js(&transcript_root(), "const canvas=document.getElementById('canvas'),c=canvas.getBoundingClientRect(),sx=c.width/canvas.width*1.25,sy=c.height/canvas.height*1.25;const frame=value.parentElement.parentElement.getBoundingClientRect();const slots=[...value.querySelectorAll('.maestro-transcript-slot')].slice(0,30);return sx!==sy && Math.abs(frame.left-c.left-12.5*sx)<1 && Math.abs(frame.top-c.top-18.25*sy)<1 && Math.abs(frame.width-800*sx)<1 && slots.every(slot=>{const child=slot.querySelector(':scope > .maestro-reading-frame');if(!child || child.querySelector('.maestro-transcript-cursor'))return true;const a=child.getBoundingClientRect(),b=slot.getBoundingClientRect();return Math.abs(a.left-b.left)<0.2 && Math.abs(a.top-b.top)<0.2})").as_bool().unwrap(), "{observation:?}");
}
/// Stable selection and owners survive unrelated streaming, resize and failed publication; only the stream retires.
/// # Panics
/// Panics if native selection, scroll, identities or separate-axis placement changes unexpectedly.
#[wasm_bindgen]
pub async fn transcript_retains_selection_scroll_and_row_identity_through_updates() {
    {
        let model = Model::new();
        let agent = model.session();
        history(&agent);
        let scene = Rc::new(Slots::new());
        scene.context.set_pixels_per_point(1.25);
        js(
            &JsValue::NULL,
            "const c=document.getElementById('canvas');c.style.width='1000px';c.style.height='450px'",
        );
        let language = Rc::new(I18n::new(Rc::new(Language)).unwrap());
        let (view, saved) = prepare(&scene, &language, &agent);
        let streams = Rc::new(RefCell::new(Vec::new()));
        let active = Rc::new(RefCell::new(None));
        let subscription = observe(
            &agent,
            (scene.clone(), language.clone(), view.clone()),
            saved.clone(),
            (active.clone(), streams.clone()),
        );
        for text in [
            "first unrelated streaming response",
            "next unrelated streaming response",
        ] {
            model.prompt(&agent, text).await;
            retained(&saved);
            assert!(active.borrow().is_none());
        }
        subscription.unsubscribe();
        retired(&streams.borrow());
        for height in [140., 180., 180.] {
            draw_in_rect(&scene, &mut view.borrow_mut(), &language, rect(height)).unwrap();
            retained(&saved);
            positioned();
        }
        frame().await;
        draw_in_rect(&scene, &mut view.borrow_mut(), &language, rect(180.)).unwrap();
        retained(&saved);
    }
    js(
        &JsValue::NULL,
        "const c=document.getElementById('canvas');c.style.width='800px';c.style.height='600px';getSelection().removeAllRanges()",
    );
    super::publication::transcript_refresh_retries_failed_slot_publication().await;
}
/// Stable history exists before either unrelated streaming turn begins.
fn history(agent: &super::Session) {
    agent.state().write().unwrap().set_messages(
        (0..30)
            .map(|n| {
                AgentMessage::User(Arc::new(RwLock::new(UserMessage {
                    content: UserContent::Text(format!("retained reader row {n}")),
                    timestamp: 0.,
                })))
            })
            .collect(),
    );
}
/// Nodes captured through the actual streaming subtree, with no private component access.
type Streams = (Rc<RefCell<Option<JsValue>>>, Rc<RefCell<Vec<JsValue>>>);
/// Observe the same real Agent producer route as the containing view, after its reducer and subscription.
fn observe(
    agent: &Rc<super::Session>,
    (scene, language, view): (Rc<Slots>, Rc<I18n>, Rc<RefCell<AgentInterface>>),
    saved: JsValue,
    (active, streams): Streams,
) -> maestro_app::Subscription<maestro_app::browser::components::messages::CustomAgentMessages> {
    agent.subscribe(Rc::new(move |event, _| {
        let event = event.read().unwrap();
        let update = matches!(*event, AgentEvent::MessageUpdate { .. });
        let end = matches!(*event, AgentEvent::MessageEnd { .. });
        drop(event);
        draw_in_rect(&scene, &mut view.borrow_mut(), &language, rect(180.)).unwrap();
        retained(&saved);
        if end && let Some(node) = active.borrow_mut().take() {
            assert!(js(&node, "return !value.isConnected").as_bool().unwrap());
            streams.borrow_mut().push(node);
        }
        let (scene, language, view, saved, active) = (
            scene.clone(),
            language.clone(),
            view.clone(),
            saved.clone(),
            active.clone(),
        );
        Box::pin(async move {
            if update {
                frame().await;
                draw_in_rect(&scene, &mut view.borrow_mut(), &language, rect(180.)).unwrap();
                retained(&saved);
                positioned();
                stream_root(&active);
                draw_in_rect(&scene, &mut view.borrow_mut(), &language, rect(180.)).unwrap();
                retained(&saved);
            }
            Ok(())
        })
    }))
}

/// Empty-start updates have no text root; subsequent updates retain the same assistant owner.
fn stream_root(active: &RefCell<Option<JsValue>>) {
    let node = js(
        &transcript_root(),
        "return value.querySelector('.maestro-transcript-slot:last-child .maestro-markdown')",
    );
    if node.is_null() {
        return;
    }
    assert!(js(&node, "const c=document.getElementById('canvas'),r=c.getBoundingClientRect(),sx=r.width/c.width*1.25;const a=value.getBoundingClientRect(),b=value.closest('.maestro-transcript-slot').getBoundingClientRect();return Math.abs(a.left-b.left-16*sx)<0.2 && Math.abs(a.top-b.top)<0.2").as_bool().unwrap());
    let mut active = active.borrow_mut();
    if let Some(before) = &*active {
        assert!(
            before == &node,
            "same response keeps its native stream root"
        );
    } else {
        *active = Some(node);
    }
}

/// Each clear retires only its response root; the next turn mounts an independent child.
fn retired(streams: &[JsValue]) {
    assert_eq!(streams.len(), 2);
    assert!(
        js(
            &js_sys::Array::from_iter(streams.iter()),
            "return value[0]!==value[1] && value.every(node=>!node.isConnected)"
        )
        .as_bool()
        .unwrap()
    );
}

/// Establish the settled scaled viewport before selecting and scrolling real native content.
fn prepare(
    scene: &Slots,
    language: &I18n,
    agent: &Rc<super::Session>,
) -> (Rc<RefCell<AgentInterface>>, JsValue) {
    maestro_web::register_message_renderer(
        "user",
        Rc::new(Renderer(Rc::new(RefCell::new(vec![])))),
    );
    let view = Rc::new(RefCell::new(AgentInterface::new(scene.host.clone())));
    view.borrow_mut().set_auto_scroll(false);
    view.borrow_mut().set_session(Some(agent.clone())).unwrap();
    draw_in_rect(scene, &mut view.borrow_mut(), language, rect(180.)).unwrap();
    draw_in_rect(scene, &mut view.borrow_mut(), language, rect(180.)).unwrap();
    positioned();
    let saved = save();
    assert!(
        js(&saved, "return value.top>0").as_bool().unwrap(),
        "{:?}",
        js(
            &transcript_root(),
            "const v=value.parentElement;return JSON.stringify({top:v.scrollTop,height:v.scrollHeight,client:v.clientHeight,style:v.getAttribute('style'),root:value.getBoundingClientRect().toJSON()})"
        )
    );
    draw_in_rect(scene, &mut view.borrow_mut(), language, rect(180.)).unwrap();
    retained(&saved);
    positioned();
    (view, saved)
}
