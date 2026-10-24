//! Conversion through the declared string kind.

use serde_json::Value;

use super::r#try::try_string;

/// Convert a supplied value to text.
pub(crate) fn from_string(value: &Value) -> Option<Value> {
    try_string::try_string(value)
}
