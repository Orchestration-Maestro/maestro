//! Current-source admission and the one-shot URL handshake.
use super::{EventSubscription, Load, Owner};
use js_sys::{Function, Object, Reflect};
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::MessageEvent;

impl Owner {
    /// Install the single actual native registration before appending the frame.
    pub(super) fn subscribe(&self) -> Result<(), JsValue> {
        let weak = Rc::downgrade(&self.state);
        let callback = Closure::new(move |event: MessageEvent| {
            if let Err(error) = receive(&weak, &event) {
                wasm_bindgen::throw_val(error);
            }
        });
        self.subscription.replace(Some(EventSubscription::new(
            js_sys::global().dyn_into()?,
            "message",
            callback,
        )?));
        Ok(())
    }
}
/// Select the actual current frame before reading or consuming a settlement.
fn receive(weak: &Weak<RefCell<Load>>, event: &MessageEvent) -> Result<(), JsValue> {
    let Some(state) = weak.upgrade() else {
        return Ok(());
    };
    let (frame, extension, id) = {
        let state = state.borrow();
        (state.frame.clone(), state.extension, state.id.clone())
    };
    let Some(window) = frame.and_then(|frame| frame.content_window()) else {
        return Ok(());
    };
    if event.source().map(JsValue::from).as_ref() != Some(window.as_ref()) {
        return Ok(());
    }
    let data = event.data();
    if !data.is_object() {
        return Ok(());
    }
    let tag = Reflect::get(&data, &"type".into())?.as_string();
    match tag.as_deref() {
        Some("open-external-url") => open(&Reflect::get(&data, &"url".into())?, extension)?,
        Some("sandbox-ready" | "sandbox-error") => {
            let pending = state.borrow_mut().pending.take();
            if let (Some(code), Some(id)) = (pending, id) {
                if tag.as_deref() == Some("sandbox-ready") {
                    let message = Object::new();
                    Reflect::set(&message, &"type".into(), &"sandbox-load".into())?;
                    Reflect::set(&message, &"sandboxId".into(), &id.into())?;
                    Reflect::set(&message, &"code".into(), &code.into())?;
                    window.post_message(&message, "*")?;
                } else {
                    let _delivery = crate::RUNTIME_MESSAGE_ROUTER
                        .with(|router| router.sandbox_error(&id, event))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}
/// Use extension tabs only on the URL path, falling back to the native window opener.
fn open(url: &JsValue, extension: bool) -> Result<(), JsValue> {
    let global = js_sys::global();
    if extension {
        let chrome = Reflect::get(&global, &"chrome".into())?;
        if !chrome.is_null() && !chrome.is_undefined() {
            let tabs = Reflect::get(&chrome, &"tabs".into())?;
            if !tabs.is_null()
                && !tabs.is_undefined()
                && let Some(create) = Reflect::get(&tabs, &"create".into())?.dyn_ref::<Function>()
            {
                let options = Object::new();
                Reflect::set(&options, &"url".into(), url)?;
                create.call1(&tabs, &options)?;
                return Ok(());
            }
        }
    }
    let open: Function = Reflect::get(&global, &"open".into())?.dyn_into()?;
    open.call2(&global, url, &"_blank".into())?;
    Ok(())
}
