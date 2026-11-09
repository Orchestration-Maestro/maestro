//! Live property access and calls on browser values, resolved at call time.
use js_sys::{Array, Function, Promise, Reflect};
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::Window;

/// Read one property as a native member access does: primitives are boxed for the
/// lookup and null or undefined throw.
pub(crate) fn get(target: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    if target.is_object() || target.is_function() {
        return Reflect::get(target, &key.into());
    }
    if target.is_null() || target.is_undefined() {
        let shown = if target.is_null() {
            "null"
        } else {
            "undefined"
        };
        let message = format!("Cannot read properties of {shown} (reading '{key}')");
        return Err(js_sys::TypeError::new(&message).into());
    }
    let boxed = invoke(
        &Reflect::get(&js_sys::global(), &"Object".into())?,
        &Array::of1(target),
    )?;
    Reflect::get(&boxed, &key.into())
}

/// Assign one property.
pub(crate) fn set(target: &JsValue, key: &str, value: &JsValue) -> Result<(), JsValue> {
    Reflect::set(target, &key.into(), value).map(drop)
}

/// Read a property through optional chaining: absent for a null or undefined receiver.
pub(crate) fn chain(target: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    if target.is_null() || target.is_undefined() {
        Ok(JsValue::UNDEFINED)
    } else {
        get(target, key)
    }
}

/// Call a named method on its receiver with the supplied arguments.
pub(crate) fn call(target: &JsValue, method: &str, arguments: &Array) -> Result<JsValue, JsValue> {
    let function = get(target, method)?;
    Reflect::apply(function.unchecked_ref::<Function>(), target, arguments)
}

/// Call a function value with an undefined receiver.
pub(crate) fn invoke(function: &JsValue, arguments: &Array) -> Result<JsValue, JsValue> {
    Reflect::apply(
        function.unchecked_ref::<Function>(),
        &JsValue::UNDEFINED,
        arguments,
    )
}

/// Coerce through the live global `String`, as the formatting helpers do.
pub(crate) fn text(value: &JsValue) -> Result<String, JsValue> {
    if let Some(text) = value.as_string() {
        return Ok(text);
    }
    let coerced = invoke(&get(&js_sys::global(), "String")?, &Array::of1(value))?;
    Ok(coerced.as_string().unwrap_or_default())
}

/// The first operand when truthy, otherwise the second, like `||`.
pub(crate) fn or(
    first: JsValue,
    second: impl FnOnce() -> Result<JsValue, JsValue>,
) -> Result<JsValue, JsValue> {
    if first.is_truthy() {
        Ok(first)
    } else {
        second()
    }
}

/// A new `Error` value carrying the message.
pub(crate) fn error(message: &str) -> JsValue {
    js_sys::Error::new(message).into()
}

/// A new `Error` for a thrown host value: `new Error(undefined)` has an empty message.
pub(crate) fn error_from(value: &JsValue) -> Result<JsValue, JsValue> {
    if value.is_undefined() {
        Ok(error(""))
    } else {
        Ok(error(&text(value)?))
    }
}

/// Await any value the way `await` does, assimilating thenables.
pub(crate) async fn wait(value: JsValue) -> Result<JsValue, JsValue> {
    JsFuture::from(Promise::resolve(&value)).await
}

/// The frame's window and its document.
pub(crate) fn page() -> Result<(Window, web_sys::Document), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    Ok((window, document))
}
