//! Conversion through the declared number kind.

use serde_json::Value;

use super::r#try::try_number;

/// Convert a supplied value to a number.
pub(crate) fn from_number(value: &Value) -> Option<Value> {
    try_number::try_number(value)
}
