//! Conversion through the declared boolean kind.

use serde_json::Value;

use super::r#try::try_boolean;

/// Convert a supplied value to a truth value.
pub(crate) fn from_boolean(value: &Value) -> Option<Value> {
    try_boolean::try_boolean(value)
}
