//! Native observers for compiler-created capture assignments.
use crate::js::{get, set};
use crate::settle::{settle, then};

/// Marker of the program scaffold removed when the program starts.
const PROGRAM: &str = "svg[data-maestro-sandbox-program]";
use js_sys::{Array, Function, Object, Promise, Reflect};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Document, Element, Event, Window};

/// A provider accessor awaiting either its capture or refused script execution.
pub(crate) struct Pending {
    /// Descriptor to restore if the script never assigns its function.
    previous: JsValue,
    /// Identity of this capture's setter.
    setter: JsValue,
    /// Invocation result returned to the ordered installation owner.
    invocation: Rc<RefCell<Result<(), JsValue>>>,
}

impl Pending {
    /// Restore a pending accessor and return its invocation outcome without overwriting provider changes.
    pub(crate) fn finish(&self, window: &Window, key: &str) -> Result<(), JsValue> {
        let current = Reflect::get_own_property_descriptor(window, &key.into())?;
        if current.is_object() && Object::is(&get(&current, "set")?, &self.setter) {
            restore(window, key, &self.previous)?;
        }
        self.invocation.borrow().clone()
    }
}

/// Invoke each captured provider after restoring its globals, while its classic script is current.
pub(crate) fn provider(window: &Window, key: &str, id: &JsValue) -> Result<Pending, JsValue> {
    let previous = Reflect::get_own_property_descriptor(window, &key.into())?;
    let (target, name, saved, id) = (window.clone(), key.to_owned(), previous.clone(), id.clone());
    let invocation = Rc::new(RefCell::new(Ok(())));
    let result = Rc::clone(&invocation);
    let setter = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
        let called = restore(&target, &name, &saved).and_then(|()| {
            Reflect::apply(
                value.unchecked_ref::<Function>(),
                &JsValue::UNDEFINED,
                &Array::of1(&id),
            )
            .map(drop)
        });
        *result.borrow_mut() = called;
    })
    .into_js_value();
    accessor(window, key, "set", &setter)?;
    Ok(Pending {
        previous,
        setter,
        invocation,
    })
}

/// Return a private outcome observer from a temporary getter, before supplied code executes.
pub(crate) fn program(window: &Window, key: &str) -> Result<(), JsValue> {
    let previous = Reflect::get_own_property_descriptor(window, &key.into())?;
    let observer = outcome(window)?;
    let listener = syntax_errors(window, key);
    let (target, name, failure) = (window.clone(), key.to_owned(), listener.clone());
    let getter = Closure::<dyn FnMut() -> Result<Object, JsValue>>::new(move || {
        target.remove_event_listener_with_callback("error", failure.unchecked_ref())?;
        restore(&target, &name, &previous)?;
        if let Some(scaffold) = target
            .document()
            .ok_or("no document")?
            .query_selector(PROGRAM)?
        {
            scaffold.remove();
        }
        Ok(observer.clone())
    });
    accessor(window, key, "get", &getter.into_js_value())?;
    window.add_event_listener_with_callback("error", listener.unchecked_ref())
}

/// Keep native completion values behind the private observer's assignment-only accessor.
fn outcome(window: &Window) -> Result<Object, JsValue> {
    let observer = Object::new();
    let target = window.clone();
    let setter =
        Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |record: JsValue| {
            let value = get(&record, "1")?;
            if get(&record, "0")?.as_bool() == Some(true) {
                success(&target, &value)?;
            } else {
                reject(&target, &Promise::reject(&value))?;
            }
            Ok(())
        });
    accessor(&observer, "value", "set", &setter.into_js_value())?;
    Ok(observer)
}

/// Route module-parser syntax errors before program entry through the same outcome observer.
fn syntax_errors(window: &Window, key: &str) -> JsValue {
    let (target, key) = (window.clone(), key.to_owned());
    Closure::<dyn FnMut(Event) -> Result<(), JsValue>>::new(move |event: Event| {
        let error = get(&event, "error")?;
        if !error.is_object() || get(&error, "name")?.as_string().as_deref() != Some("SyntaxError")
        {
            return Ok(());
        }
        let observer = get(&target, &key)?;
        set(&observer, "value", &Array::of2(&false.into(), &error))?;
        event.prevent_default();
        Ok(())
    })
    .into_js_value()
}

/// Assimilate returned thenables, including rejection of the awaited successful completion.
fn success(window: &Window, value: &JsValue) -> Result<(), JsValue> {
    let resolved = window.clone();
    let finish = Closure::once_into_js(move |value: JsValue| settle(&resolved, true, value));
    let pending = then(&Promise::resolve(value), &finish, &JsValue::UNDEFINED)?;
    reject(window, &pending)
}

/// Observe one rejection without retrying a failing error completion.
fn reject(window: &Window, pending: &JsValue) -> Result<(), JsValue> {
    let rejected = window.clone();
    let fail = Closure::once_into_js(move |error: JsValue| settle(&rejected, false, error));
    then(pending, &JsValue::UNDEFINED, &fail).map(drop)
}

/// Install a configurable native getter or setter.
fn accessor(target: &Object, key: &str, field: &str, callback: &JsValue) -> Result<(), JsValue> {
    let descriptor = Object::new();
    set(&descriptor, field, callback)?;
    set(&descriptor, "configurable", &true.into())?;
    if !Reflect::define_property(target, &key.into(), &descriptor)? {
        return Err(js_sys::TypeError::new("cannot install capture accessor").into());
    }
    Ok(())
}

/// Restore the previous descriptor or remove a newly created slot.
fn restore(window: &Window, key: &str, previous: &JsValue) -> Result<(), JsValue> {
    let restored = if previous.is_undefined() {
        Reflect::delete_property(window.unchecked_ref::<Object>(), &key.into())?
    } else {
        Reflect::define_property(window, &key.into(), previous.unchecked_ref::<Object>())?
    };
    if !restored {
        return Err(js_sys::TypeError::new("cannot restore capture slot").into());
    }
    Ok(())
}

/// Capture a supplied provider from its parser-created script and invoke it synchronously in Rust.
pub(crate) fn run_carrier(
    window: &Window,
    document: &Document,
    template: &Element,
    id: &JsValue,
) -> Result<(), JsValue> {
    let script: Element = get(
        &get(&get(template, "content")?, "firstChild")?,
        "firstChild",
    )?
    .dyn_into()?;
    let head: Element = get(document, "head")?.dyn_into()?;
    let key = template
        .get_attribute("data-maestro-capture")
        .ok_or("no capture key")?;
    let pending = provider(window, &key, id)?;
    let result = head.append_child(&script);
    script.remove();
    template.remove();
    let finished = pending.finish(window, &key);
    result?;
    finished
}

/// Settle a program that could not be compiled as the browser would report its syntax error.
pub(crate) fn refuse(window: &Window, message: &str) -> Result<(), JsValue> {
    let error = js_sys::SyntaxError::new(message);
    settle(window, false, error.into()).map(drop)
}
