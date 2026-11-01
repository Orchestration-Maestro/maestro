//! Conversion through the single constant a literal node declares.

use serde_json::Value;

use super::{r#try::try_boolean, r#try::try_number, r#try::try_string};
use crate::arguments::validation::authored::schema_view::View;

/// Convert a supplied value through a literal node of the supplied schema.
pub(crate) fn from_literal_view(view: View<'_>, value: &mut Value) -> Option<Value> {
    view.raw
        .get("const")
        .and_then(|constant| from_literal(constant, value))
}

/// Convert a supplied value through the single value a literal declares.
///
/// The value converts only when it converts to that very constant; otherwise the supplied
/// value stays as it arrived and the later checking pass reports the mismatch.
pub(crate) fn from_literal(constant: &Value, value: &Value) -> Option<Value> {
    if equal(constant, value) {
        return None;
    }
    let converted = match constant {
        Value::String(_) => try_string::try_string(value),
        Value::Number(_) => try_number::try_number(value),
        Value::Bool(_) => try_boolean::try_boolean(value),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }?;
    equal(constant, &converted).then_some(converted)
}

/// Whether a supplied value is the declared constant, comparing numbers by value.
fn equal(constant: &Value, value: &Value) -> bool {
    crate::arguments::validation::scalars::equal(constant, value)
}
