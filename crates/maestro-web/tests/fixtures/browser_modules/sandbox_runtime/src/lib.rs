//! Prototype sandbox runtime bundle that qualifies the isolated-frame bootstrap.
//!
//! Built separately with its own no-modules binding glue. The start function reads
//! the inert configuration block, assigns the sandbox identifier and every data
//! global, then installs each runtime selection in order. A supplied provider
//! function waits in an inert carrier until its selection is reached. Native
//! assignment observers capture providers and settle compiled program outcomes.
use js_sys::{Array, Function, JSON, Object, Promise, Reflect};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{Document, Element, Window};

mod capture;

/// Ordered installation trace the qualification documents read.
const TRACE: &str = "__sandboxInstallTrace";
/// Marker of the inert configuration block.
const CONFIGURATION: &str = "script[data-maestro-sandbox]";
/// Marker of a supplied provider carrier.
const CARRIER: &str = "data-maestro-sandbox-provider";
/// Marker of the program scaffold removed when the program starts.
const PROGRAM: &str = "svg[data-maestro-sandbox-program]";

/// Read one property.
fn get(target: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    Reflect::get(target, &key.into())
}

/// Assign one property.
fn set(target: &JsValue, key: &str, value: &JsValue) -> Result<(), JsValue> {
    Reflect::set(target, &key.into(), value).map(drop)
}

/// Call a named method with the supplied arguments.
fn call(target: &JsValue, method: &str, arguments: &Array) -> Result<JsValue, JsValue> {
    let function = get(target, method)?;
    Reflect::apply(function.unchecked_ref::<Function>(), target, arguments)
}

/// The current window and its document.
fn page() -> Result<(Window, Document), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    Ok((window, document))
}

/// Install from the configuration block; staged documents replace it first.
///
/// # Errors
/// Returns a missing configuration, malformed data or a failed installation.
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let (window, document) = page()?;
    let block = document
        .query_selector(CONFIGURATION)?
        .ok_or("no configuration")?;
    let config = JSON::parse(&block.text_content().unwrap_or_default())?;
    if let Some(html) = get(&config, "staged")?.as_string() {
        return stage(&window, &document, &config, &html);
    }
    let carriers = carriers(&block);
    install(&window, &document, &config, carriers)?;
    if let Some(boot) = get(&document, "currentScript")?.dyn_ref::<Element>() {
        boot.remove();
    }
    block.remove();
    Ok(())
}

/// Collect the provider carriers that precede the configuration, in document order.
fn carriers(block: &Element) -> Vec<Element> {
    let mut found = Vec::new();
    let mut current = block.previous_element_sibling();
    while let Some(element) = current {
        if !element.has_attribute(CARRIER) {
            break;
        }
        current = element.previous_element_sibling();
        found.push(element);
    }
    found.reverse();
    found
}

/// Assign identifier and data before any selection, then install selections in order.
fn install(
    window: &Window,
    document: &Document,
    config: &JsValue,
    carriers: Vec<Element>,
) -> Result<(), JsValue> {
    set(window, "sandboxId", &get(config, "sandboxId")?)?;
    let trace = Array::new();
    set(window, TRACE, &trace)?;
    let globals: Object = get(config, "globals")?.dyn_into()?;
    for key in Object::keys(&globals).iter() {
        Reflect::set(window, &key, &Reflect::get(&globals, &key)?)?;
    }
    let mut carriers = carriers.into_iter();
    for selection in Array::from(&get(config, "runtime")?).iter() {
        match selection.as_string().as_deref() {
            Some("attachments") => {
                install_attachments(window)?;
                let data = get(window, "attachments")?.js_typeof();
                let entry = format!("attachments:{}", data.as_string().unwrap_or_default());
                trace.push(&JsValue::from(entry));
            }
            Some("provider") => run_carrier(
                window,
                document,
                &carriers.next().ok_or("no carrier")?,
                &get(config, "providerId")?,
            )?,
            _ => return Err("unknown runtime selection".into()),
        }
    }
    if let Some(key) = get(config, "programCapture")?.as_string() {
        capture::program(window, &key)?;
    }
    Ok(())
}

/// Capture a supplied provider from its parser-created script and invoke it synchronously in Rust.
fn run_carrier(
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
    let pending = capture::provider(window, &key, id)?;
    let result = head.append_child(&script);
    script.remove();
    template.remove();
    let finished = pending.finish(window, &key);
    result?;
    finished
}

/// Install the synchronous attachment listing over the live attachment global.
fn install_attachments(window: &Window) -> Result<(), JsValue> {
    let target = window.clone();
    let list = Closure::<dyn Fn() -> Result<Array, JsValue>>::new(move || {
        let listed = Array::new();
        for item in Array::from(&get(&target, "attachments")?).iter() {
            let entry = Object::new();
            for key in ["id", "fileName", "mimeType", "size"] {
                set(&entry, key, &get(&item, key)?)?;
            }
            listed.push(&entry);
        }
        Ok(listed)
    });
    set(window, "listAttachments", &list.into_js_value())
}

/// Replace the staging document only after the runtime has installed, then tell the parent.
fn stage(
    window: &Window,
    document: &Document,
    config: &JsValue,
    html: &str,
) -> Result<(), JsValue> {
    let trace = |entry: &str| -> Result<(), JsValue> {
        get(window, TRACE)?
            .unchecked_into::<Array>()
            .push(&JsValue::from_str(entry));
        Ok(())
    };
    call(document, "open", &Array::new())?;
    install(window, document, config, Vec::new())?;
    trace("staged")?;
    if let Err(error) = call(document, "write", &Array::of1(&JsValue::from_str(html))) {
        trace(&format!(
            "write-refused:{}",
            get(&error, "name")?.as_string().unwrap_or_default()
        ))?;
    }
    call(document, "close", &Array::new())?;
    let message = Object::new();
    set(&message, "type", &"sandbox-staged".into())?;
    set(&message, "trace", &get(window, TRACE)?)?;
    window
        .parent()?
        .ok_or("no parent")?
        .post_message(&message, "*")
}

/// Attach fulfillment and rejection reactions through the native `then`.
fn then(promise: &JsValue, resolved: &JsValue, rejected: &JsValue) -> Result<JsValue, JsValue> {
    call(promise, "then", &Array::of2(resolved, rejected))
}

/// Await completion callbacks, then hand the outcome to the completion global.
fn settle(window: &Window, success: bool, outcome: JsValue) -> Result<JsValue, JsValue> {
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
        Closure::<dyn Fn(JsValue) -> Result<JsValue, JsValue>>::new(move |callback: JsValue| {
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
