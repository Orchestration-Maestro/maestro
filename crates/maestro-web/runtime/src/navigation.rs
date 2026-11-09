//! Interception of link clicks, form submissions and location assignment.
use crate::js::{call, chain, get, page, set};
use js_sys::{Array, Object, Reflect};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{AddEventListenerOptions, Document, Window};

/// Route external navigation to the parent window.
///
/// # Errors
/// Returns the native failure of a listener registration. A refused location
/// interception is tolerated.
pub(crate) fn install(window: &Window) -> Result<(), JsValue> {
    let (_, document) = page()?;
    let target = window.clone();
    let click = Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |event: JsValue| {
        let Some(url) = link(&event)? else {
            return Ok(());
        };
        if url.starts_with("http://") || url.starts_with("https://") {
            suppress(&event)?;
            open(&target, &url.into())?;
        }
        Ok(())
    });
    capture(&document, "click", &click.into_js_value())?;
    let target = window.clone();
    let submit =
        Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |event: JsValue| {
            let form = get(&event, "target")?;
            let action = chain(&form, "action")?;
            if form.is_truthy() && action.is_truthy() {
                suppress(&event)?;
                open(&target, &action)?;
            }
            Ok(())
        });
    capture(&document, "submit", &submit.into_js_value())?;
    // The location accessor is unforgeable in current browsers; a refusal is expected.
    drop(intercept_location(window));
    Ok(())
}

/// Register a capturing listener.
fn capture(document: &Document, kind: &str, listener: &JsValue) -> Result<(), JsValue> {
    let options = AddEventListenerOptions::new();
    options.set_capture(true);
    document.add_event_listener_with_callback_and_add_event_listener_options(
        kind,
        listener.unchecked_ref(),
        &options,
    )
}

/// The effective address of the hyperlink containing the click target, if any.
fn link(event: &JsValue) -> Result<Option<String>, JsValue> {
    let anchor = call(&get(event, "target")?, "closest", &Array::of1(&"a".into()))?;
    if anchor.is_null() || !call(&anchor, "matches", &Array::of1(&":any-link".into()))?.is_truthy()
    {
        return Ok(None);
    }
    let href = get(&anchor, "href")?;
    if let Some(address) = href.as_string() {
        return Ok(Some(address));
    }
    let global = js_sys::global();
    let base = get(&anchor, "baseURI")?;
    let parsed = call(
        &get(&global, "URL")?,
        "parse",
        &Array::of2(&get(&href, "animVal")?, &base),
    )?;
    Ok(chain(&parsed, "href")?.as_string())
}

/// Stop the default action and further propagation.
fn suppress(event: &JsValue) -> Result<(), JsValue> {
    call(event, "preventDefault", &Array::new())?;
    call(event, "stopPropagation", &Array::new()).map(drop)
}

/// Ask the parent window to open the address.
fn open(window: &Window, url: &JsValue) -> Result<(), JsValue> {
    let message = Object::new();
    set(&message, "type", &"open-external-url".into())?;
    set(&message, "url", url)?;
    let parent = get(window, "parent")?;
    call(&parent, "postMessage", &Array::of2(&message, &"*".into())).map(drop)
}

/// Redirect assignments of `window.location` to the parent window.
fn intercept_location(window: &Window) -> Result<(), JsValue> {
    let original = get(window, "location")?;
    let descriptor = Object::new();
    let read = Closure::<dyn FnMut() -> JsValue>::new(move || original.clone());
    set(&descriptor, "get", &read.into_js_value())?;
    let target = window.clone();
    let write = Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |url: JsValue| {
        open(&target, &crate::js::text(&url)?.into())
    });
    set(&descriptor, "set", &write.into_js_value())?;
    Reflect::define_property(window, &"location".into(), &descriptor).map(drop)
}
