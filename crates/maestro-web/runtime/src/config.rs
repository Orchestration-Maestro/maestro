//! Reading the configuration block and installing its selections in order.
use crate::js::{call, get, page, set};
use crate::{artifacts, attachments, bridge, capture, console, download, navigation};
use js_sys::{Array, JSON, Object, Reflect};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Document, Element, Window};

/// Marker of the inert configuration block.
const CONFIGURATION: &str = "script[data-maestro-sandbox]";
/// Marker of a supplied provider carrier.
const CARRIER: &str = "data-maestro-sandbox-provider";

/// Install from the document's configuration block; a document without one is left alone.
pub(crate) fn install_document() -> Result<(), JsValue> {
    let Ok((window, document)) = page() else {
        return Ok(());
    };
    let Some(block) = document.query_selector(CONFIGURATION)? else {
        return Ok(());
    };
    let config = JSON::parse(&block.text_content().unwrap_or_default())?;
    let carriers = carriers(&block);
    install(&window, &document, &config, carriers)?;
    if let Some(boot) = get(&document, "currentScript")?.dyn_ref::<Element>() {
        boot.remove();
    }
    block.remove();
    Ok(())
}

/// Install one message bridge in a world without a prepared document.
pub(crate) fn install_role(role: &str, id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    match role {
        "frame-bridge" => bridge::install_frame(&window, &id.into()),
        _ => Err(format!("unknown bridge role: {role}").into()),
    }
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

/// Assign identifier and data first, then install in the order the consumers observe.
fn install(
    window: &Window,
    document: &Document,
    config: &JsValue,
    carriers: Vec<Element>,
) -> Result<(), JsValue> {
    let id = get(config, "sandboxId")?;
    set(window, "sandboxId", &id)?;
    for pair in Array::from(&get(config, "globals")?).iter() {
        let pair = Array::from(&pair);
        Reflect::set(window, &pair.get(0), &pair.get(1))?;
    }
    if let Some(name) = get(config, "completion")?.as_string() {
        expose_completion(window, &name)?;
    }
    let standalone = get(config, "standalone")?.is_truthy();
    if !standalone {
        bridge::install_frame(window, &id)?;
    }
    let mut carriers = carriers.into_iter();
    for selection in Array::from(&get(config, "runtime")?).iter() {
        match selection.as_string().as_deref() {
            Some("artifacts") => artifacts::install(window)?,
            Some("attachments") => attachments::install(window)?,
            Some("console") => console::install(window)?,
            Some("fileDownload") => download::install(window)?,
            Some("provider") => {
                capture::run_carrier(window, document, &carriers.next().ok_or("no carrier")?, &id)?;
            }
            _ => return Err("unknown runtime selection".into()),
        }
    }
    if !standalone {
        navigation::install(window)?;
    }
    if let Some(key) = get(config, "program")?.as_string() {
        capture::program(window, &key)?;
    }
    if let Some(message) = get(config, "programError")?.as_string() {
        capture::refuse(window, &message)?;
    }
    Ok(())
}

/// Define the property whose read, by a script after the whole source, completes the run.
fn expose_completion(window: &Window, name: &str) -> Result<(), JsValue> {
    let target = window.clone();
    let signal = Closure::<dyn FnMut() -> Result<(), JsValue>>::new(move || {
        if get(&target, "complete")?.is_truthy() {
            call(&target, "complete", &Array::new())?;
        }
        Ok(())
    });
    let descriptor = Object::new();
    set(&descriptor, "get", &signal.into_js_value())?;
    set(&descriptor, "configurable", &true.into())?;
    Reflect::define_property(window, &name.into(), &descriptor).map(drop)
}
