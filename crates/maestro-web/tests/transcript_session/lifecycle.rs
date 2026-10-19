//! Containing-owner release through native registrations and real Agent subscriptions.
use super::{Model, frame};
use crate::{
    reading_slots_support::{Slots, js},
    transcript_browser::aggregate::{Language, draw},
};
use maestro_web::{AgentInterface, I18n};
use std::rc::Rc;
use wasm_bindgen::prelude::*;

/// Native registrations and session ownership end with their containing view.
/// # Panics
/// Panics if one instance leaks, retires another instance or cannot reconnect.
#[wasm_bindgen]
pub async fn session_disconnect_releases_observers_listeners_and_subscription() {
    instrument();
    let scene = Slots::new();
    let language = I18n::new(Rc::new(Language)).unwrap();
    let model = Model::new();
    let agent = model.session();
    model.prompt(&agent, "retained session").await;
    let mut first = AgentInterface::new(scene.host.clone());
    first.set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut first, &language);
    let baseline = registrations();
    assert!(baseline.0 > 0. && baseline.1 > 0.);
    let retired = js(
        &JsValue::NULL,
        "return document.querySelector('.maestro-transcript')",
    );
    first.set_session(None).unwrap();
    assert_eq!(registrations(), (0., 0.));
    assert!(!js(&retired, "return value.isConnected").as_bool().unwrap());
    first.set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut first, &language);
    assert_eq!(registrations(), baseline);
    let mut second = AgentInterface::new(scene.host.clone());
    second.set_session(Some(agent.clone())).unwrap();
    draw(&scene, &mut second, &language);
    let both = registrations();
    assert_eq!(both, (baseline.0 * 2., baseline.1 * 2.));
    let sentinel = Rc::new(());
    let weak_sentinel = Rc::downgrade(&sentinel);
    first.on_cost_click = Some(Rc::new(move || {
        let _ = &sentinel;
    }));
    drop(first);
    assert!(
        weak_sentinel.upgrade().is_none(),
        "session observers cannot retain the view"
    );
    assert_eq!(registrations(), baseline);
    model.prompt(&agent, "after first release").await;
    draw(&scene, &mut second, &language);
    assert!(super::text().contains("after first release"));
    drop(second);
    assert_eq!(registrations(), (0., 0.));
    model.prompt(&agent, "after all views released").await;
    frame().await;
    assert_eq!(registrations(), (0., 0.));
    let weak = Rc::downgrade(&agent);
    drop(agent);
    assert!(
        weak.upgrade().is_none(),
        "released subscriptions cannot retain the session"
    );
    restore();
}
/// Track the actual native observer and listener handles without replacing their behavior.
fn instrument() {
    js(
        &JsValue::NULL,
        r"
        globalThis.lifecycle={observers:new Set(),scroll:new Map(),Resize:ResizeObserver,add:EventTarget.prototype.addEventListener,remove:EventTarget.prototype.removeEventListener};
        globalThis.ResizeObserver=class extends lifecycle.Resize {
            observe(target, ...args){lifecycle.observers.add(this);return super.observe(target,...args)}
            disconnect(){lifecycle.observers.delete(this);return super.disconnect()}
        };
        EventTarget.prototype.addEventListener=function(name,callback,...args){
            if(name==='scroll'){let callbacks=lifecycle.scroll.get(this);if(!callbacks){callbacks=new Set();lifecycle.scroll.set(this,callbacks)}callbacks.add(callback)}
            return lifecycle.add.call(this,name,callback,...args)
        };
        EventTarget.prototype.removeEventListener=function(name,callback,...args){
            if(name==='scroll')lifecycle.scroll.get(this)?.delete(callback);
            return lifecycle.remove.call(this,name,callback,...args)
        };
    ",
    );
}
/// Observe active registrations, not declared cleanup flags.
fn registrations() -> (f64, f64) {
    let counts = js(
        &JsValue::NULL,
        "return [lifecycle.observers.size,[...lifecycle.scroll.values()].reduce((sum,set)=>sum+set.size,0)]",
    );
    let counts = js_sys::Array::from(&counts);
    (
        counts.get(0).as_f64().unwrap(),
        counts.get(1).as_f64().unwrap(),
    )
}
/// Return the actual browser methods after the containing owners have been released.
fn restore() {
    js(
        &JsValue::NULL,
        "globalThis.ResizeObserver=lifecycle.Resize;EventTarget.prototype.addEventListener=lifecycle.add;EventTarget.prototype.removeEventListener=lifecycle.remove;delete globalThis.lifecycle",
    );
}
