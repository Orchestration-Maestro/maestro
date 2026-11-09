//! Settlement of a program outcome: completion callbacks, then the completion global.
use crate::js::{call, get, set};
use js_sys::{Array, Function, Object, Promise, Reflect};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::Window;

/// Attach fulfillment and rejection reactions through the native `then`.
pub(crate) fn then(
    promise: &JsValue,
    resolved: &JsValue,
    rejected: &JsValue,
) -> Result<JsValue, JsValue> {
    call(promise, "then", &Array::of2(resolved, rejected))
}

/// Await completion callbacks, then hand the outcome to the completion global.
pub(crate) fn settle(window: &Window, success: bool, outcome: JsValue) -> Result<JsValue, JsValue> {
    let callbacks = get(window, "__completionCallbacks")?;
    let joined = match callbacks.dyn_ref::<Array>() {
        Some(list) if list.length() > 0 => join(window, list, success),
        _ => Promise::resolve(&JsValue::UNDEFINED),
    };
    let target = window.clone();
    let finish = Closure::once_into_js(move |_: JsValue| complete(&target, success, &outcome));
    then(&joined, &finish, &JsValue::UNDEFINED)
}

/// Map callbacks natively, skipping elisions and stopping on a synchronous throw.
fn join(window: &Window, list: &Array, success: bool) -> Promise {
    let invoke =
        Closure::<dyn FnMut(JsValue) -> Result<JsValue, JsValue>>::new(move |callback: JsValue| {
            Reflect::apply(
                callback.unchecked_ref::<Function>(),
                &JsValue::UNDEFINED,
                &Array::of1(&success.into()),
            )
        });
    let results: Array = match call(list, "map", &Array::of1(&invoke.into_js_value())) {
        Ok(values) => values.unchecked_into(),
        Err(error) => {
            report(window, &error);
            return Promise::resolve(&JsValue::UNDEFINED);
        }
    };
    let target = window.clone();
    let failed = Closure::once_into_js(move |error: JsValue| report(&target, &error));
    then(&Promise::all(&results), &JsValue::UNDEFINED, &failed).map_or_else(
        |_| Promise::resolve(&JsValue::UNDEFINED),
        wasm_bindgen::JsCast::unchecked_into,
    )
}

/// Log a callback failure without replacing the program outcome.
fn report(window: &Window, error: &JsValue) {
    let console = get(window, "console").unwrap_or(JsValue::UNDEFINED);
    // A failed log has nowhere to go and must not replace the program outcome.
    let _ = call(
        &console,
        "error",
        &Array::of2(&"Completion callback error:".into(), error),
    );
}

/// Call the live completion global with a returned value or an error projection.
fn complete(window: &Window, success: bool, outcome: &JsValue) -> Result<JsValue, JsValue> {
    if success {
        return call(window, "complete", &Array::of2(&JsValue::NULL, outcome));
    }
    let field = |key: &str| -> Result<JsValue, JsValue> {
        if outcome.is_object() || outcome.is_function() {
            get(outcome, key)
        } else {
            Ok(JsValue::UNDEFINED)
        }
    };
    let message = field("message")?;
    let message = if message.is_truthy() {
        message
    } else {
        call(&js_sys::global(), "String", &Array::of1(outcome))?
    };
    let stack = field("stack")?;
    let stack = if stack.is_truthy() {
        stack
    } else {
        get(&js_sys::Error::new(""), "stack")?
    };
    let projection = Object::new();
    set(&projection, "message", &message)?;
    set(&projection, "stack", &stack)?;
    call(window, "complete", &Array::of1(&projection))
}
