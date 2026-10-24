//! Conversion through the declared integer kind.

use serde_json::Value;

use super::r#try::try_number;

/// Convert a supplied value to a number without a fractional part.
pub(crate) fn from_integer(value: &Value) -> Option<Value> {
    try_number::try_integer(value)
}
