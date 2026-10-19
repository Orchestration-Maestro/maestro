//! Reached containing-slot publication failure retries authoritative geometry.
use super::Model;
use crate::{
    reading_slots_support::{Slots, js},
    transcript_browser::aggregate::{Language, draw, try_draw},
};
use maestro_web::{AgentInterface, I18n};
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, prelude::*};

/// A failed slot-size transaction is not acknowledged as the latest successful frame.
/// # Panics
/// Panics if publication failure loses invalidation or replaces retained native roots.
#[wasm_bindgen]
pub async fn transcript_refresh_retries_failed_slot_publication() {
    let model = Model::new();
    let agent = model.session();
    let scene = Slots::new();
    let language = I18n::new(Rc::new(Language)).unwrap();
    model.prompt(&agent, "before publication").await;
    let mut view = AgentInterface::new(scene.host.clone());
    view.set_auto_scroll(false);
    view.set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut view, &language);
    let root: web_sys::Element = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')",
    )
    .dyn_into()
    .unwrap();
    let saved = js(
        &root,
        "const p=value.querySelector('p'),range=document.createRange();range.selectNodeContents(p);getSelection().removeAllRanges();getSelection().addRange(range);return {root:value,first:value.querySelector('.maestro-transcript-slot'),p,anchor:getSelection().anchorNode,offset:getSelection().anchorOffset,top:value.parentElement.scrollTop}",
    );
    model
        .prompt(
            &agent,
            "after publication\n\nsecond paragraph\n\nthird paragraph",
        )
        .await;
    js(
        &JsValue::NULL,
        "globalThis.slotRejects=0;globalThis.slotAttribute=Element.prototype.setAttribute;Element.prototype.setAttribute=function(key,value){if(key==='style' && this.classList.contains('maestro-transcript-slot')){Element.prototype.setAttribute=slotAttribute;slotRejects++;throw Error('controlled slot publication failure')}return slotAttribute.call(this,key,value)}",
    );
    assert!(try_draw(&scene, &mut view, &language).is_err());
    assert_eq!(js(&JsValue::NULL, "return slotRejects").as_f64(), Some(1.));
    assert!(js(&saved, "return value.root.isConnected && value.first===value.root.querySelector('.maestro-transcript-slot') && value.p.isConnected && getSelection().anchorNode===value.anchor && getSelection().anchorOffset===value.offset && value.root.parentElement.scrollTop===value.top").as_bool().unwrap());
    draw(&scene, &mut view, &language);
    assert!(js(&root, "const slots=[...value.querySelectorAll('.maestro-transcript-slot')];return slots.length===4 && value.innerText.includes('after publication') && slots.every(slot=>slot.getBoundingClientRect().height>0 && Math.abs(slot.firstElementChild.getBoundingClientRect().top-slot.getBoundingClientRect().top)<0.1)").as_bool().unwrap());
}
