//! Pointer admission through the native containing transcript.
use super::aggregate::{Language, Renderer, draw, draw_frame};
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

/// A real pointer reaches the canvas copy control through its native ancestors.
/// # Panics
/// Panics if native containment captures the pointer or copying never starts.
#[wasm_bindgen]
pub async fn transcript_native_ancestors_admit_canvas_copy_pointer() {
    let scene = Slots::new();
    let i18n = I18n::new(Rc::new(Language)).unwrap();
    maestro_web::register_message_renderer(
        "user",
        Rc::new(Renderer(Rc::new(RefCell::new(vec![])))),
    );
    let agent = Rc::new(Agent::<BrowserMessages<NoCustomMessages>>::default());
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![AgentMessage::User(Arc::new(RwLock::new(
            maestro_app::UserMessage {
                content: maestro_app::UserContent::Text(
                    "```text\ncopy through ancestors\n```".into(),
                ),
                timestamp: 0.,
            },
        )))]);
    let mut interface = AgentInterface::new(scene.host.clone());
    interface.set_auto_scroll(false);
    interface.set_session(Some(agent)).unwrap();
    draw(&scene, &mut interface, &i18n);
    draw(&scene, &mut interface, &i18n);
    let saved = js(
        &JsValue::NULL,
        "globalThis.transcriptPointers=[];globalThis.transcriptCopies=[];const canvas=document.getElementById('canvas');globalThis.transcriptCapture=e=>transcriptPointers.push([e.clientX,e.clientY,e.type==='pointerdown']);canvas.addEventListener('pointerdown',transcriptCapture);canvas.addEventListener('pointerup',transcriptCapture);const saved=Object.getOwnPropertyDescriptor(navigator,'clipboard');Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText(text){transcriptCopies.push(text);return Promise.resolve()}}});return saved",
    );
    let promise = js(
        &JsValue::NULL,
        "const r=document.querySelector('.maestro-transcript .control-slot').getBoundingClientRect();globalThis.transcriptHit=document.elementFromPoint(r.x+r.width/2,r.y+r.height/2)===document.getElementById('canvas');console.log('APERTURE-ROOT '+transcriptHit);return globalThis.transcriptPointer()",
    );
    wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise))
        .await
        .unwrap();
    deliver_pointer(&scene, &mut interface, &i18n);
    let copied = js(&JsValue::NULL, "return transcriptCopies.join('|')");
    let admitted = js(&JsValue::NULL, "return transcriptHit");
    js(
        &saved,
        "const canvas=document.getElementById('canvas');canvas.removeEventListener('pointerdown',transcriptCapture);canvas.removeEventListener('pointerup',transcriptCapture);if(value)Object.defineProperty(navigator,'clipboard',value);else delete navigator.clipboard",
    );
    assert_eq!(
        admitted.as_bool(),
        Some(true),
        "native ancestors must expose the canvas aperture"
    );
    assert_eq!(
        copied.as_string().as_deref(),
        Some("copy through ancestors\n")
    );
}

/// Feed only the pointer endpoints the actual canvas received.
fn deliver_pointer(scene: &Slots, interface: &mut AgentInterface, i18n: &I18n) {
    let events = js(&JsValue::NULL, "return JSON.stringify(transcriptPointers)")
        .as_string()
        .unwrap();
    let events: Vec<(f32, f32, bool)> = serde_json::from_str(&events).unwrap();
    for (x, y, pressed) in events {
        draw_frame(
            scene,
            interface,
            i18n,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800., 180.)),
            vec![
                egui::Event::PointerMoved(egui::pos2(x, y)),
                egui::Event::PointerButton {
                    pos: egui::pos2(x, y),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .unwrap()
        .textures_delta
        .clear();
    }
}
