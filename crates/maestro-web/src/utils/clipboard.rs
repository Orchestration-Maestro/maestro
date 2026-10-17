//! Native clipboard routing with scoped fallback resources.
use js_sys::{Function, Promise, Reflect};
use std::{future::Future, pin::Pin};
use wasm_bindgen::{JsCast as _, JsValue};
/// A temporary native selection, removed even when a reached effect fails.
struct Selection(web_sys::HtmlTextAreaElement);
impl Drop for Selection {
    fn drop(&mut self) {
        self.0.remove();
    }
}
/// Resolve and call a fallible native method, retaining its receiver.
fn call(receiver: &JsValue, name: &str, arg: &JsValue) -> Result<JsValue, JsValue> {
    Reflect::get(receiver, &name.into())?
        .dyn_into::<Function>()?
        .call1(receiver, arg)
}
/// Copy through a temporary native textarea in the host's actual document.
fn fallback(document: &web_sys::Document, text: &str) -> bool {
    let attempt = || -> Result<bool, JsValue> {
        let selection = Selection(document.create_element("textarea")?.dyn_into()?);
        selection.0.set_value(text);
        selection
            .0
            .set_attribute("style", "position: fixed; opacity: 0")?;
        document
            .body()
            .ok_or(JsValue::NULL)?
            .append_child(&selection.0)?;
        selection.0.select();
        Ok(call(document.as_ref(), "execCommand", &"copy".into())?
            .as_bool()
            .unwrap_or(false))
    };
    attempt().unwrap_or(false)
}
/// Start the API call synchronously, returning only owned settlement work.
pub(crate) fn copy_to_clipboard(
    document: &web_sys::Document,
    text: &str,
) -> Pin<Box<dyn Future<Output = bool>>> {
    let attempt = || -> Result<Promise, JsValue> {
        let window = document.default_view().ok_or(JsValue::NULL)?;
        let clipboard = Reflect::get(window.navigator().as_ref(), &"clipboard".into())?;
        call(&clipboard, "writeText", &text.into())?.dyn_into()
    };
    if let Ok(promise) = attempt() {
        let document = document.clone();
        let text = text.to_owned();
        Box::pin(async move {
            wasm_bindgen_futures::JsFuture::from(promise).await.is_ok()
                || fallback(&document, &text)
        })
    } else {
        let success = fallback(document, text);
        Box::pin(async move { success })
    }
}
