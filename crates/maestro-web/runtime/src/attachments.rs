//! Synchronous attachment helpers over the live attachment global.
use crate::js::{call, error, get, or, set, text};
use js_sys::{Array, Uint8Array};
use wasm_bindgen::{JsValue, closure::Closure};
use web_sys::Window;

/// Install `listAttachments`, `readTextAttachment` and `readBinaryAttachment`.
///
/// # Errors
/// Returns the native failure of any assignment.
pub(crate) fn install(window: &Window) -> Result<(), JsValue> {
    let target = window.clone();
    let list = Closure::<dyn FnMut() -> Result<JsValue, JsValue>>::new(move || {
        let project = Closure::<dyn FnMut(JsValue) -> Result<JsValue, JsValue>>::new(
            |attachment: JsValue| {
                let entry = js_sys::Object::new();
                for key in ["id", "fileName", "mimeType", "size"] {
                    set(&entry, key, &get(&attachment, key)?)?;
                }
                Ok(entry.into())
            },
        );
        call(
            &attachments(&target)?,
            "map",
            &Array::of1(&project.into_js_value()),
        )
    });
    set(window, "listAttachments", &list.into_js_value())?;
    let target = window.clone();
    let read_text =
        Closure::<dyn FnMut(JsValue) -> Result<JsValue, JsValue>>::new(move |id: JsValue| {
            let attachment = find(&target, &id)?;
            let extracted = get(&attachment, "extractedText")?;
            if extracted.is_truthy() {
                return Ok(extracted);
            }
            decode(&target, &attachment).map_err(|_| {
                error(&format!(
                    "Failed to decode text content for: {}",
                    text(&id).unwrap_or_default()
                ))
            })
        });
    set(window, "readTextAttachment", &read_text.into_js_value())?;
    let target = window.clone();
    let read_binary =
        Closure::<dyn FnMut(JsValue) -> Result<JsValue, JsValue>>::new(move |id: JsValue| {
            let binary = decode(&target, &find(&target, &id)?)?;
            let bytes: Vec<u8> = binary
                .as_string()
                .unwrap_or_default()
                .chars()
                .map(|unit| u8::try_from(u32::from(unit)).unwrap_or_default())
                .collect();
            Ok(Uint8Array::from(bytes.as_slice()).into())
        });
    set(window, "readBinaryAttachment", &read_binary.into_js_value())
}

/// The live attachment array, or an empty one when the global is falsy.
fn attachments(window: &Window) -> Result<JsValue, JsValue> {
    or(get(window, "attachments")?, || Ok(Array::new().into()))
}

/// The first attachment whose id is strictly equal to the requested one.
fn find(window: &Window, id: &JsValue) -> Result<JsValue, JsValue> {
    let wanted = id.clone();
    let matches =
        Closure::<dyn FnMut(JsValue) -> Result<bool, JsValue>>::new(move |candidate: JsValue| {
            Ok(get(&candidate, "id")? == wanted)
        });
    let found = call(
        &attachments(window)?,
        "find",
        &Array::of1(&matches.into_js_value()),
    )?;
    if found.is_undefined() {
        return Err(error(&format!("Attachment not found: {}", text(id)?)));
    }
    Ok(found)
}

/// Native base64 decoding of the attachment content into a binary string.
fn decode(window: &Window, attachment: &JsValue) -> Result<JsValue, JsValue> {
    call(window, "atob", &Array::of1(&get(attachment, "content")?))
}
