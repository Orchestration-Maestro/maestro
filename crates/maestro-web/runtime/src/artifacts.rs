//! Artifact helpers: online through the message bridge, offline over the snapshot global.
use crate::js::{call, error, error_from, get, invoke, or, set, text, wait};
use js_sys::{Array, JSON, Object, Promise};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::future_to_promise;
use web_sys::Window;

/// One artifact operation request, built field by field.
fn operation(action: &str, fields: &[(&str, &JsValue)]) -> Result<Object, JsValue> {
    let message = Object::new();
    set(&message, "type", &"artifact-operation".into())?;
    set(&message, "action", &action.into())?;
    for (key, value) in fields {
        set(&message, key, value)?;
    }
    Ok(message)
}

/// The bridge's sender when present, read at call time.
fn sender(window: &Window) -> Result<Option<JsValue>, JsValue> {
    let send = get(window, "sendRuntimeMessage")?;
    Ok(send.is_truthy().then_some(send))
}

/// Send one operation and reject with the host's error when it did not succeed.
async fn ask(send: &JsValue, message: Object) -> Result<JsValue, JsValue> {
    let response = wait(invoke(send, &Array::of1(&message))?).await?;
    if get(&response, "success")?.is_truthy() {
        Ok(response)
    } else {
        Err(error_from(&get(&response, "error")?)?)
    }
}

/// The offline snapshot, or an empty object when the global is falsy.
fn snapshot(window: &Window) -> Result<JsValue, JsValue> {
    or(get(window, "artifacts")?, || Ok(Object::new().into()))
}

/// Install `listArtifacts`, `getArtifact`, `createOrUpdateArtifact` and `deleteArtifact`.
///
/// # Errors
/// Returns the native failure of any assignment.
pub(crate) fn install(window: &Window) -> Result<(), JsValue> {
    let target = window.clone();
    let list = Closure::<dyn FnMut() -> Promise>::new(move || {
        let window = target.clone();
        future_to_promise(async move {
            match sender(&window)? {
                Some(send) => get(&ask(&send, operation("list", &[])?).await?, "result"),
                None => Ok(Object::keys(snapshot(&window)?.unchecked_ref::<Object>()).into()),
            }
        })
    });
    set(window, "listArtifacts", &list.into_js_value())?;
    let target = window.clone();
    let read = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |filename: JsValue| {
        let window = target.clone();
        future_to_promise(async move { read(&window, filename).await })
    });
    set(window, "getArtifact", &read.into_js_value())?;
    let target = window.clone();
    let write = Closure::<dyn FnMut(JsValue, JsValue, JsValue) -> Promise>::new(
        move |filename: JsValue, content: JsValue, mime_type: JsValue| {
            let window = target.clone();
            future_to_promise(async move { write(&window, filename, content, mime_type).await })
        },
    );
    set(window, "createOrUpdateArtifact", &write.into_js_value())?;
    let target = window.clone();
    let delete = Closure::<dyn FnMut(JsValue) -> Promise>::new(move |filename: JsValue| {
        let window = target.clone();
        future_to_promise(async move {
            let Some(send) = sender(&window)? else {
                return Err(error("Cannot delete artifacts in offline mode (read-only)"));
            };
            ask(&send, operation("delete", &[("filename", &filename)])?).await?;
            Ok(JsValue::UNDEFINED)
        })
    });
    set(window, "deleteArtifact", &delete.into_js_value())
}

/// Read one artifact and parse names ending in lowercase `.json`.
async fn read(window: &Window, filename: JsValue) -> Result<JsValue, JsValue> {
    let content = if let Some(send) = sender(window)? {
        let message = operation("get", &[("filename", &filename)])?;
        get(&ask(&send, message).await?, "result")?
    } else {
        let artifacts = snapshot(window)?;
        if !Object::has_own(artifacts.unchecked_ref::<Object>(), &filename) {
            return Err(error(&format!(
                "Artifact not found (offline mode): {}",
                text(&filename)?
            )));
        }
        js_sys::Reflect::get(&artifacts, &filename)?
    };
    if !call(&filename, "endsWith", &Array::of1(&".json".into()))?.is_truthy() {
        return Ok(content);
    }
    JSON::parse(&text(&content)?).map_err(|failure| {
        error(&format!(
            "Failed to parse JSON from {}: {}",
            text(&filename).unwrap_or_default(),
            text(&failure).unwrap_or_default()
        ))
    })
}

/// Create or update one artifact, serializing non-string content once.
async fn write(
    window: &Window,
    filename: JsValue,
    content: JsValue,
    mime_type: JsValue,
) -> Result<JsValue, JsValue> {
    let Some(send) = sender(window)? else {
        return Err(error(
            "Cannot create/update artifacts in offline mode (read-only)",
        ));
    };
    let mut serialized = content;
    if serialized.as_string().is_none() {
        serialized =
            JSON::stringify_with_replacer_and_space(&serialized, &JsValue::NULL, &2.into())?.into();
    }
    let fields = [
        ("filename", &filename),
        ("content", &serialized),
        ("mimeType", &mime_type),
    ];
    ask(&send, operation("createOrUpdate", &fields)?).await?;
    Ok(JsValue::UNDEFINED)
}
