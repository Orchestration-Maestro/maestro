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
    let thrown = Rc::new(RefCell::new(None));
    let handler = ErrorHandler::install(window, &thrown)?;
    let result = head.append_child(&script);
    let restored = handler.restore(window);
    script.remove();
    template.remove();
    let finished = pending.finish(window, &key);
    result?;
    restored?;
    finished?;
    thrown.take().map_or(Ok(()), Err)
}

/// The window's error handler property while the carrier script runs.
///
/// The observer owns the native handler slot, so no listener is registered and supplied code cannot
/// disable it. Reads and writes of the property reach `current` instead, which the observer forwards
/// each event to and which the property gets back afterwards.
struct ErrorHandler {
    /// The property's native descriptor, put back once the script has run.
    native: JsValue,
    /// The handler supplied code sees and sets; it is null when none is set.
    current: Rc<RefCell<JsValue>>,
}

impl ErrorHandler {
    /// Route the window's error events to an observer that records the first thrown error.
    fn install(window: &Window, thrown: &Rc<RefCell<Option<JsValue>>>) -> Result<Self, JsValue> {
        let native = Reflect::get_own_property_descriptor(window, &"onerror".into())?;
        let current = Rc::new(RefCell::new(get(window, "onerror")?));
        let reader = Rc::clone(&current);
        let getter = Closure::<dyn FnMut() -> JsValue>::new(move || reader.borrow().clone());
        let writer = Rc::clone(&current);
        let setter = Closure::<dyn FnMut(JsValue)>::new(move |value: JsValue| {
            *writer.borrow_mut() = if value.is_function() {
                value
            } else {
                JsValue::NULL
            };
        });
        let observer = script_errors(window, thrown, &current);
        accessor(window, "onerror", "get", &getter.into_js_value())?;
        accessor(window, "onerror", "set", &setter.into_js_value())?;
        Reflect::apply(
            get(&native, "set")?.unchecked_ref::<Function>(),
            window,
            &Array::of1(&observer),
        )?;
        Ok(Self { native, current })
    }

    /// Put the native property back, holding the handler as supplied code left it.
    fn restore(&self, window: &Window) -> Result<(), JsValue> {
        let native = self.native.unchecked_ref::<Object>();
        if !Reflect::define_property(window, &"onerror".into(), native)? {
            return Err(js_sys::TypeError::new("cannot restore error handler").into());
        }
        set(window, "onerror", &self.current.borrow())
    }
}

/// Record the first error the carrier script throws while it runs, then pass the event to the
/// handler supplied code set, whose result decides whether the browser reports it.
fn script_errors(
    window: &Window,
    thrown: &Rc<RefCell<Option<JsValue>>>,
    current: &Rc<RefCell<JsValue>>,
) -> JsValue {
    let (window, thrown, current) = (window.clone(), Rc::clone(thrown), Rc::clone(current));
    Closure::<dyn FnMut(JsValue, JsValue, JsValue, JsValue, JsValue) -> Result<JsValue, JsValue>>::new(
        move |message, source, line, column, error: JsValue| {
            thrown.borrow_mut().get_or_insert(error.clone());
            let handler = current.borrow().clone();
            if !handler.is_function() {
                return Ok(JsValue::UNDEFINED);
            }
            let event = Array::of5(&message, &source, &line, &column, &error);
            Reflect::apply(handler.unchecked_ref::<Function>(), &window, &event)
        },
    )
    .into_js_value()
}

/// Settle a program that could not be compiled as the browser would report its syntax error.
pub(crate) fn refuse(window: &Window, message: &str) -> Result<(), JsValue> {
    let error = js_sys::SyntaxError::new(message);
    settle(window, false, error.into()).map(drop)
}
