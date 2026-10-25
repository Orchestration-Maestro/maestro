//! Literal native child projection with finite active-iterator traversal.
use crate::{browser::sandbox_value::native_string, tools::types::ToolUiError};
use wasm_bindgen::{JsCast as _, JsValue};

/// One suspended iterator or value, keeping Rust traversal and drop depth flat.
enum Work {
    /// A child awaiting projection.
    Value(JsValue),
    /// Iterator whose object stays active until it is exhausted.
    Resume(js_sys::Object, js_sys::IntoIter<JsValue>),
}
/// Convert primitive/object values literally while expanding finite iterable children.
pub(super) fn value_text(value: &JsValue) -> Result<String, ToolUiError> {
    let active = js_sys::WeakSet::<js_sys::Object>::new();
    let mut pending = vec![Work::Value(value.clone())];
    let mut text = String::new();
    while let Some(work) = pending.pop() {
        match work {
            Work::Value(value) => {
                if value.is_null() || value.is_undefined() {
                    continue;
                }
                if !value.is_object() {
                    text.push_str(
                        &native_string(&value).map_err(|error| ToolUiError::native(&error))?,
                    );
                    continue;
                }
                let object: js_sys::Object = value.clone().unchecked_into();
                if active.has(&object) {
                    continue;
                }
                if let Some(iter) =
                    js_sys::try_iter(&value).map_err(|error| ToolUiError::native(&error))?
                {
                    active.add(&object);
                    pending.push(Work::Resume(object, iter));
                } else {
                    text.push_str(
                        &native_string(&value).map_err(|error| ToolUiError::native(&error))?,
                    );
                }
            }
            Work::Resume(object, mut iter) => {
                if let Some(value) = iter.next() {
                    pending.push(Work::Resume(object, iter));
                    pending.push(Work::Value(
                        value.map_err(|error| ToolUiError::native(&error))?,
                    ));
                } else {
                    active.delete(&object);
                }
            }
        }
    }
    Ok(text)
}
