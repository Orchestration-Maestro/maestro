//! Retained native sandbox values and browser-owned display semantics.
use js_sys::{Function, JSON, Reflect};
use maestro_app::DiagnosticErrorInfo;
use maestro_app::browser::components::sandboxed_iframe::SandboxValue;
use std::borrow::Cow;
use wasm_bindgen::{JsCast, JsValue};

/// An owned native handle; cloning retains the same underlying value.
#[derive(Clone, Debug)]
pub struct BrowserSandboxValue(pub JsValue);
impl SandboxValue for BrowserSandboxValue {
    fn format_repl_value(&self) -> Result<Cow<'_, str>, DiagnosticErrorInfo> {
        let display = if self.0.is_object() || self.0.is_null() {
            JSON::stringify_with_replacer_and_space(&self.0, &JsValue::UNDEFINED, &2.into())
                .map(|value| value.as_string().unwrap_or_else(|| "undefined".into()))
        } else {
            native_string(&self.0)
        };
        display.map(Cow::Owned).map_err(|error| diagnostic(&error))
    }
}
/// Invoke the browser's primitive string conversion without a JSON round trip.
fn native_string(value: &JsValue) -> Result<String, JsValue> {
    let string: Function = Reflect::get(&js_sys::global(), &"String".into())?.dyn_into()?;
    Ok(string
        .call1(&JsValue::UNDEFINED, value)?
        .as_string()
        .unwrap_or_default())
}
/// Project supplied native exception fields into the existing diagnostic record.
pub(crate) fn diagnostic(value: &JsValue) -> DiagnosticErrorInfo {
    let field = |name: &str| {
        Reflect::get(value, &name.into())
            .ok()
            .and_then(|field| field.as_string())
    };
    DiagnosticErrorInfo {
        name: field("name"),
        message: field("message").unwrap_or_else(|| format!("{value:?}")),
        stack: field("stack"),
        code: None,
    }
}
