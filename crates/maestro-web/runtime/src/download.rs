//! Byte-preserving file returns: through the bridge online, as a browser download offline.
use crate::js::{call, error, error_from, get, invoke, page, set, wait};
use js_sys::{Array, JSON, Promise, Uint8Array};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::future_to_promise;
use web_sys::{Blob, BlobPropertyBag, Window};

/// Install `returnDownloadableFile`.
///
/// # Errors
/// Returns the native failure of the assignment.
pub(crate) fn install(window: &Window) -> Result<(), JsValue> {
    let target = window.clone();
    let returned = Closure::<dyn FnMut(JsValue, JsValue, JsValue) -> Promise>::new(
        move |name: JsValue, content: JsValue, mime_type: JsValue| {
            let window = target.clone();
            future_to_promise(async move { deliver(&window, name, content, mime_type).await })
        },
    );
    set(window, "returnDownloadableFile", &returned.into_js_value())
}

/// A MIME type is required unless the content determines one.
fn required(kind: &str) -> JsValue {
    error(&format!(
        "returnDownloadableFile: MIME type is required for {kind} content. Please provide a mimeType parameter (e.g., 'image/png')."
    ))
}

/// Select the transferred content and MIME type by content kind.
async fn describe(content: JsValue, mime_type: &JsValue) -> Result<(JsValue, JsValue), JsValue> {
    if let Some(blob) = content.dyn_ref::<Blob>() {
        let buffer = wait(call(blob, "arrayBuffer", &Array::new())?).await?;
        let kind: JsValue = blob.type_().into();
        if !mime_type.is_truthy() && !kind.is_truthy() {
            return Err(required("Blob"));
        }
        let mime = crate::js::or(mime_type.clone(), || {
            crate::js::or(kind, || Ok("application/octet-stream".into()))
        })?;
        return Ok((Uint8Array::new(&buffer).into(), mime));
    }
    if content.is_instance_of::<Uint8Array>() {
        if !mime_type.is_truthy() {
            return Err(required("Uint8Array"));
        }
        return Ok((content, mime_type.clone()));
    }
    if content.as_string().is_some() {
        let mime = crate::js::or(mime_type.clone(), || Ok("text/plain".into()))?;
        return Ok((content, mime));
    }
    let json = JSON::stringify_with_replacer_and_space(&content, &JsValue::NULL, &2.into())?;
    let mime = crate::js::or(mime_type.clone(), || Ok("application/json".into()))?;
    Ok((json.into(), mime))
}

/// Return one file to the host, or download it directly without a bridge.
async fn deliver(
    window: &Window,
    name: JsValue,
    content: JsValue,
    mime_type: JsValue,
) -> Result<JsValue, JsValue> {
    let (content, mime) = describe(content, &mime_type).await?;
    let send = get(window, "sendRuntimeMessage")?;
    if send.is_truthy() {
        let message = js_sys::Object::new();
        set(&message, "type", &"file-returned".into())?;
        set(&message, "fileName", &name)?;
        set(&message, "content", &content)?;
        set(&message, "mimeType", &mime)?;
        let response = wait(invoke(&send, &Array::of1(&message))?).await?;
        let failure = get(&response, "error")?;
        if failure.is_truthy() {
            return Err(error_from(&failure)?);
        }
        return Ok(JsValue::UNDEFINED);
    }
    download(window, &name, &content, &mime)?;
    Ok(JsValue::UNDEFINED)
}

/// Offline mode: click a temporary anchor over an object URL, then revoke it.
fn download(
    window: &Window,
    name: &JsValue,
    content: &JsValue,
    mime: &JsValue,
) -> Result<(), JsValue> {
    let options = BlobPropertyBag::new();
    set(&options, "type", mime)?;
    let blob = Blob::new_with_u8_array_sequence_and_options(&Array::of1(content), &options)?;
    let url = get(window, "URL")?;
    let location = call(&url, "createObjectURL", &Array::of1(&blob))?;
    let (_, document) = page()?;
    let anchor = document.create_element("a")?;
    set(&anchor, "href", &location)?;
    set(&anchor, "download", name)?;
    call(&anchor, "click", &Array::new())?;
    call(&url, "revokeObjectURL", &Array::of1(&location)).map(drop)
}
