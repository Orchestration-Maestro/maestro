//! Native JSON conversion shared by browser consumers.
use serde_json::Value;
use std::io;
use wasm_bindgen::JsValue;

/// Parse an application JSON value without a handwritten native-value codec.
pub(crate) fn encode_json(
    value: &Value,
    native_error: fn(&JsValue) -> io::Error,
) -> io::Result<JsValue> {
    let text = serde_json::to_string(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    js_sys::JSON::parse(&text).map_err(|error| native_error(&error))
}
