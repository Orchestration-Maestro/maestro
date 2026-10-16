//! Repeated cursor progress within the common request settlement owner.
use wasm_bindgen::{JsCast, JsValue};

/// Result progression selected when a native request is registered.
pub(super) enum Progress {
    /// One success settles an ordinary request.
    Single,
    /// Cursor successes accumulate native values until exhaustion.
    Cursor(js_sys::Array),
}
impl Progress {
    /// Return a final value only when the request has completed.
    pub(super) fn advance(&mut self, value: JsValue) -> Result<Option<JsValue>, JsValue> {
        match self {
            Self::Single => Ok(Some(value)),
            Self::Cursor(values) => {
                if value.is_null() || value.is_undefined() {
                    return Ok(Some(values.clone().into()));
                }
                let cursor: web_sys::IdbCursorWithValue = value.dyn_into()?;
                values.push(&cursor.value()?);
                cursor.continue_()?;
                Ok(None)
            }
        }
    }
}
