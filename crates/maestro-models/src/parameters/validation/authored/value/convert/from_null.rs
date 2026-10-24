//! Conversion through the declared null kind.

use serde_json::Value;

use super::r#try::try_null;

/// Convert a supplied value to the JSON null value.
pub(crate) fn from_null(value: &Value) -> Option<Value> {
    try_null::try_null(value)
}
