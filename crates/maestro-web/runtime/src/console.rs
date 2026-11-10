//! Console forwarding, error capture and one-shot completion.
use crate::js::{call, chain, get, invoke, or, set, text, wait};
use js_sys::{Array, JSON, Object, Promise, Proxy};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::future_to_promise;
use web_sys::Window;

/// The console methods that are wrapped.
const METHODS: [&str; 4] = ["log", "error", "warn", "info"];

/// Per-installation state shared by the wrappers, listeners and completion.
#[derive(Default)]
struct State {
    /// Send promises the completion registration waits for.
    pending: RefCell<Vec<JsValue>>,
    /// The latest uncaught error or unhandled rejection as `{ message, stack }`.
    last_error: RefCell<Option<JsValue>>,
    /// Whether `complete` has already been called.
    completed: Cell<bool>,
}

/// Install the wrappers, completion registration, error listeners and `complete`.
///
/// # Errors
/// Returns the native failure of any assignment or listener registration.
pub(crate) fn install(window: &Window) -> Result<(), JsValue> {
    let originals = originals(window)?;
    let state = Rc::new(State::default());
    let console = get(&js_sys::global(), "console")?;
    for method in METHODS {
        let wrapper = wrap(window, method, get(&originals, method)?, &state)?;
        set(&console, method, &wrapper)?;
    }
    register_flush(window, &state)?;
    listen(window, &state)?;
    expose_complete(window, &state)
}

/// The first bound console methods, kept so repeated installation never stacks wrappers.
fn originals(window: &Window) -> Result<JsValue, JsValue> {
    let existing = get(window, "__originalConsole")?;
    if existing.is_truthy() {
        return Ok(existing);
    }
    let console = get(&js_sys::global(), "console")?;
    let bound = Object::new();
    for method in METHODS {
        let function = get(&console, method)?;
        set(
            &bound,
            method,
            &call(&function, "bind", &Array::of1(&console))?,
        )?;
    }
    set(window, "__originalConsole", &bound)?;
    Ok(bound.into())
}

/// One wrapper accepting any number of arguments, built as a native call interceptor.
fn wrap(
    window: &Window,
    method: &'static str,
    original: JsValue,
    state: &Rc<State>,
) -> Result<JsValue, JsValue> {
    let (window, state) = (window.clone(), Rc::clone(state));
    let trap = Closure::<dyn Fn(JsValue, JsValue, Array) -> Result<JsValue, JsValue>>::new(
        move |_target, _receiver, args: Array| {
            let line = join_text(&args)?;
            let console = get(&js_sys::global(), "console")?;
            call(&original, "apply", &Array::of2(&console, &args))?;
            forward(&window, &state, method, &line, &args)?;
            Ok(JsValue::UNDEFINED)
        },
    );
    let handler = Object::new();
    set(&handler, "apply", &trap.into_js_value())?;
    let target = Closure::<dyn FnMut()>::new(|| {}).into_js_value();
    Ok(Proxy::new(&target, &handler).into())
}

/// Space-joined native formatting: compact JSON for objects, `String` otherwise.
fn join_text(args: &Array) -> Result<JsValue, JsValue> {
    let parts = Array::new();
    for arg in args.iter() {
        let line = if arg.js_typeof().as_string().as_deref() == Some("object") {
            JSON::stringify(&arg)
                .map(JsValue::from)
                .or_else(|_| text(&arg).map(JsValue::from))?
        } else {
            text(&arg)?.into()
        };
        parts.push(&line);
    }
    call(&parts, "join", &Array::of1(&" ".into()))
}

/// Send one console entry through the bridge when it exists, tracking its settlement.
fn forward(
    window: &Window,
    state: &State,
    method: &str,
    line: &JsValue,
    args: &Array,
) -> Result<(), JsValue> {
    let send = get(window, "sendRuntimeMessage")?;
    if !send.is_truthy() {
        return Ok(());
    }
    let message = Object::new();
    set(&message, "type", &"console".into())?;
    set(&message, "method", &method.into())?;
    set(&message, "text", line)?;
    set(&message, "args", args)?;
    let ignore = Closure::<dyn FnMut(JsValue)>::new(|_| {}).into_js_value();
    let settled = call(
        &invoke(&send, &Array::of1(&message))?,
        "catch",
        &Array::of1(&ignore),
    )?;
    state.pending.borrow_mut().push(settled);
    Ok(())
}

/// Wait for every pending send when the completion registration exists.
fn register_flush(window: &Window, state: &Rc<State>) -> Result<(), JsValue> {
    let register = get(window, "onCompleted")?;
    if !register.is_truthy() {
        return Ok(());
    }
    let state = Rc::clone(state);
    let flush = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |_success: JsValue| {
        let pending: Array = state.pending.borrow().iter().collect();
        future_to_promise(async move {
            if pending.length() > 0 {
                wait(Promise::all(&pending).into()).await?;
            }
            Ok(JsValue::UNDEFINED)
        })
    });
    invoke(&register, &Array::of1(&flush.into_js_value())).map(drop)
}

/// Record the latest error without adding a console entry.
fn listen(window: &Window, state: &Rc<State>) -> Result<(), JsValue> {
    let held = Rc::clone(state);
    let on_error =
        Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |event: JsValue| {
            let error = get(&event, "error")?;
            let owner = event_text(&event)?;
            let message = or(chain(&error, "message")?, || {
                or(get(&event, "message")?, || text(&event).map(JsValue::from))
            })?;
            let stack = or(chain(&error, "stack")?, || Ok(owner.into()))?;
            held.last_error.replace(Some(report(&message, &stack)?));
            Ok(())
        });
    window.add_event_listener_with_callback("error", on_error.into_js_value().unchecked_ref())?;
    let held = Rc::clone(state);
    let on_rejection =
        Closure::<dyn FnMut(JsValue) -> Result<(), JsValue>>::new(move |event: JsValue| {
            let reason = get(&event, "reason")?;
            let detail = or(chain(&reason, "message")?, || {
                or(reason.clone(), || Ok("Unknown error".into()))
            })?;
            let line = format!("Unhandled promise rejection: {}", text(&detail)?);
            let message = or(chain(&reason, "message")?, || {
                or(text(&reason).map(JsValue::from)?, || {
                    Ok("Unhandled promise rejection".into())
                })
            })?;
            let stack = or(chain(&reason, "stack")?, || Ok(line.into()))?;
            held.last_error.replace(Some(report(&message, &stack)?));
            Ok(())
        });
    window.add_event_listener_with_callback(
        "unhandledrejection",
        on_rejection.into_js_value().unchecked_ref(),
    )
}

/// The stack text of an error event with its source position.
fn event_text(event: &JsValue) -> Result<String, JsValue> {
    let error = get(event, "error")?;
    let head = or(chain(&error, "stack")?, || {
        or(get(event, "message")?, || text(event).map(JsValue::from))
    })?;
    let line = or(get(event, "lineno")?, || Ok("?".into()))?;
    let column = or(get(event, "colno")?, || Ok("?".into()))?;
    Ok(format!(
        "{} at line {}:{}",
        text(&head)?,
        text(&line)?,
        text(&column)?
    ))
}

/// A `{ message, stack }` projection.
fn report(message: &JsValue, stack: &JsValue) -> Result<JsValue, JsValue> {
    let record = Object::new();
    set(&record, "message", message)?;
    set(&record, "stack", stack)?;
    Ok(record.into())
}

/// Expose `complete(error, returnValue)`, admitting only its first call.
fn expose_complete(window: &Window, state: &Rc<State>) -> Result<(), JsValue> {
    let (target, state) = (window.clone(), Rc::clone(state));
    let complete = Closure::<dyn FnMut(JsValue, JsValue) -> Promise>::new(
        move |error: JsValue, value: JsValue| {
            let (window, state) = (target.clone(), Rc::clone(&state));
            let first = !state.completed.replace(true);
            future_to_promise(async move {
                if first {
                    finish(&window, &state, error, value).await?;
                }
                Ok(JsValue::UNDEFINED)
            })
        },
    );
    set(window, "complete", &complete.into_js_value())
}

/// Send the execution outcome when a bridge exists.
async fn finish(
    window: &Window,
    state: &State,
    error: JsValue,
    value: JsValue,
) -> Result<(), JsValue> {
    let recorded = state
        .last_error
        .borrow()
        .clone()
        .unwrap_or(JsValue::UNDEFINED);
    let failure = or(error, || Ok(recorded))?;
    let send = get(window, "sendRuntimeMessage")?;
    if !send.is_truthy() {
        return Ok(());
    }
    let message = Object::new();
    if failure.is_truthy() {
        set(&message, "type", &"execution-error".into())?;
        set(&message, "error", &failure)?;
    } else {
        set(&message, "type", &"execution-complete".into())?;
        set(&message, "returnValue", &value)?;
    }
    wait(invoke(&send, &Array::of1(&message))?).await.map(drop)
}
