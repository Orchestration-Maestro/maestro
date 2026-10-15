//! Browser-native localized date display.
use serde_json::Value;
use std::io;
use wasm_bindgen::JsValue;
/// Preserve the native date override without supplying a locale.
/// # Errors
/// Returns invalid data for an unsupported timestamp domain.
pub fn localize_header_date(timestamp: &Value) -> io::Result<String> {
    let value =
        match timestamp {
            Value::String(text) => JsValue::from_str(text),
            Value::Number(number) => JsValue::from_f64(number.as_f64().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "unsupported timestamp")
            })?),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported timestamp",
                ));
            }
        };
    let date = js_sys::Date::new(&value);
    let object: &js_sys::Object = date.as_ref();
    Ok(object.to_locale_string().into())
}
