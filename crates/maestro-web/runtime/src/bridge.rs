//! Completion registration and the message transports of the sandbox: the frame-to-parent
//! channel and the extension user-script channel.
use crate::js::{call, error, get, invoke, set, wait};
use js_sys::{Array, Date, Math, Object, Promise};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::future_to_promise;
use web_sys::{MessageEvent, Window};

/// Milliseconds a frame request waits for its reply.
const DEADLINE: i32 = 30_000;

/// Reply listener retained until its request settles or expires.
type Handler = Closure<dyn FnMut(MessageEvent) -> Result<(), JsValue>>;
/// The listener's single owner, emptied on release.
type Slot = Rc<RefCell<Option<Handler>>>;

/// Shared completion registration plus the frame-to-parent transport.
///
/// # Errors
/// Returns the native failure of any assignment.
pub(crate) fn install_frame(window: &Window, sandbox_id: &JsValue) -> Result<(), JsValue> {
    completion(window)?;
    let target = window.clone();
    let id = sandbox_id.clone();
    let send = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |message: JsValue| {
        request(&target, &id, &message)
    });
    set(window, "sendRuntimeMessage", &send.into_js_value())
}

/// Shared completion registration plus the extension transport of a user script.
///
/// # Errors
/// Returns the native failure of any assignment.
pub(crate) fn install_user_script(window: &Window, sandbox_id: &JsValue) -> Result<(), JsValue> {
    completion(window)?;
    let id = sandbox_id.clone();
    let send = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |message: JsValue| {
        extension_request(id.clone(), message)
    });
    set(window, "sendRuntimeMessage", &send.into_js_value())
}

/// One request to the extension runtime, settled with its native reply: the message is copied
/// with the sandbox identifier last, so the identifier always wins.
fn extension_request(id: JsValue, message: JsValue) -> Promise {
    future_to_promise(async move {
        let payload = Object::assign(&Object::new(), message.unchecked_ref::<Object>());
        set(&payload, "sandboxId", &id)?;
        let runtime = get(&get(&js_sys::global(), "chrome")?, "runtime")?;
        wait(call(&runtime, "sendMessage", &Array::of1(&payload))?).await
    })
}

/// Reset the public callback list and install `onCompleted` as an append to its current value.
fn completion(window: &Window) -> Result<(), JsValue> {
    set(window, "__completionCallbacks", &Array::new())?;
    let target = window.clone();
    let register =
        Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |callback: JsValue| {
            call(
                &get(&target, "__completionCallbacks")?,
                "push",
                &Array::of1(&callback),
            )
            .map(drop)
        });
    set(window, "onCompleted", &register.into_js_value())
}

/// One correlated request: the listener is installed before the post.
fn request(window: &Window, id: &JsValue, message: &JsValue) -> Promise {
    let (window, id, message) = (window.clone(), id.clone(), message.clone());
    Promise::new(&mut |resolve, reject| {
        if let Err(failure) = post(&window, &id, &message, &resolve, &reject) {
            drop(invoke(&reject, &Array::of1(&failure)));
        }
    })
}

/// Correlation tag of a request: a timestamp and a base-36 random suffix.
fn message_id() -> Result<JsValue, JsValue> {
    let random: JsValue = Math::random().into();
    let digits = call(&random, "toString", &Array::of1(&36.into()))?;
    let suffix = call(&digits, "substring", &Array::of2(&2.into(), &9.into()))?;
    Ok(format!(
        "msg_{:.0}_{}",
        Date::now(),
        suffix.as_string().unwrap_or_default()
    )
    .into())
}

/// Install the reply listener, post the request, then arm the deadline.
fn post(
    window: &Window,
    id: &JsValue,
    message: &JsValue,
    resolve: &js_sys::Function,
    reject: &js_sys::Function,
) -> Result<(), JsValue> {
    let tag = message_id()?;
    let slot = Slot::default();
    let handler = reply(
        window,
        &tag,
        resolve.clone(),
        reject.clone(),
        Rc::clone(&slot),
    );
    window.add_event_listener_with_callback("message", handler.as_ref().unchecked_ref())?;
    *slot.borrow_mut() = Some(handler);
    let payload = Object::assign(&Object::new(), message.unchecked_ref::<Object>());
    set(&payload, "sandboxId", id)?;
    set(&payload, "messageId", &tag)?;
    let parent = get(window, "parent")?;
    call(&parent, "postMessage", &Array::of2(&payload, &"*".into()))?;
    let (target, expired) = (window.clone(), reject.clone());
    let deadline = Closure::once_into_js(move || {
        release(&target, &slot);
        invoke(
            &expired.into(),
            &Array::of1(&error("Runtime message timeout")),
        )
        .map(drop)
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(
        deadline.unchecked_ref(),
        DEADLINE,
    )?;
    Ok(())
}

/// Listener that settles the request whose reply carries the matching tag.
fn reply(
    window: &Window,
    tag: &JsValue,
    resolve: js_sys::Function,
    reject: js_sys::Function,
    slot: Slot,
) -> Handler {
    let (target, tag) = (window.clone(), tag.clone());
    Closure::new(move |event: MessageEvent| {
        let data = event.data();
        if get(&data, "type")?.as_string().as_deref() != Some("runtime-response")
            || get(&data, "messageId")? != tag
        {
            return Ok(());
        }
        release(&target, &slot);
        if get(&data, "success")?.is_truthy() {
            return resolve.call1(&JsValue::UNDEFINED, &data).map(drop);
        }
        let reason = crate::js::or(get(&data, "error")?, || Ok("Operation failed".into()))?;
        let failure = js_sys::Error::new(&crate::js::text(&reason)?);
        reject.call1(&JsValue::UNDEFINED, &failure).map(drop)
    })
}

/// Detach the reply listener and free it once no call of it can still be running.
fn release(window: &Window, slot: &Slot) {
    if let Some(handler) = slot.borrow_mut().take() {
        // A failed detach has nowhere to report; the listener is unreachable either way.
        let _ =
            window.remove_event_listener_with_callback("message", handler.as_ref().unchecked_ref());
        let free = Closure::once_into_js(move || drop(handler));
        window.queue_microtask(free.unchecked_ref());
    }
}
