//! Retained array normalization and separate legacy argument folding.

use serde_json::{Value, json};

/// Parse array text in place; fold a complete legacy pair only in the returned value.
pub(super) fn prepare_edit_arguments(args: &mut Value) -> Value {
    let Value::Object(object) = args else {
        return args.clone();
    };
    if let Some(Value::String(text)) = object.get("edits")
        && let Ok(parsed @ Value::Array(_)) = serde_json::from_str::<Value>(text)
    {
        object.insert("edits".into(), parsed);
    }
    let mut prepared = args.clone();
    let Value::Object(object) = &mut prepared else {
        return prepared;
    };
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
    prepared
}
