//! Owned argument preparation before shared validation.

use serde_json::{Value, json};

/// Parse array text and fold a complete legacy replacement pair, without I/O.
pub(super) fn prepare_edit_arguments(mut args: Value) -> Value {
    let Value::Object(object) = &mut args else {
        return args;
    };
    if let Some(Value::String(text)) = object.get("edits")
        && let Ok(parsed @ Value::Array(_)) = serde_json::from_str::<Value>(text)
    {
        object.insert("edits".into(), parsed);
    }
    if object.get("oldText").is_some_and(Value::is_string)
        && object.get("newText").is_some_and(Value::is_string)
    {
        let old = object.shift_remove("oldText");
        let new = object.shift_remove("newText");
        let pair = json!({"oldText": old, "newText": new});
        match object.get_mut("edits") {
            Some(Value::Array(edits)) => edits.push(pair),
            _ => {
                object.insert("edits".into(), Value::Array(vec![pair]));
            }
        }
    }
    args
}
