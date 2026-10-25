//! Defaults a provider applies to replayed tool-call arguments.

use crate::JsonObject;
use maestro_json::is_truthy;
use serde_json::Value;

/// Replace a null argument root with an empty object, keeping every other root.
///
/// Providers whose source states a nullish default send an empty object for absent
/// arguments; a supplied `false`, `0`, empty string, array or object is its own value.
#[must_use]
pub(crate) fn nullish_default(arguments: Value) -> Value {
    if arguments.is_null() {
        JsonObject::new().into()
    } else {
        arguments
    }
}

/// Write replayed arguments as compact text under a truthy default.
///
/// Providers whose source states a truthy default send an empty object for null,
/// `false`, zero and the empty string; an empty array or object is truthy and stays.
///
/// # Errors
/// Returns the canonical compact writer's JSON error.
pub(crate) fn truthy_text(arguments: &Value) -> Result<String, serde_json::Error> {
    if is_truthy(arguments) {
        maestro_json::compact_json(arguments)
    } else {
        maestro_json::compact_json(&JsonObject::new().into())
    }
}
