//! Declared literal, enumerated and alternative conversions.

use serde_json::Value;

use super::scalars;
use super::validates;

/// Convert through the single value a literal node declares.
///
/// A supplied value converts only when it converts to that very constant; otherwise it keeps
/// the value it arrived with, and the later checking pass reports the mismatch.
pub(super) fn convert_literal(value: &mut Value, schema: &Value) {
    let Some(constant) = schema.get("const") else {
        return;
    };
    let mut candidate = value.clone();
    convert_member(&mut candidate, constant);
    if equals(&candidate, constant) {
        *value = candidate;
    }
}

/// Convert through the alternatives the enumerated constants name, in declaration order.
pub(super) fn convert_enum(value: &mut Value, schema: &Value) {
    let constants = schema
        .get("enum")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(selected) = select(value, &constants) {
        *value = selected;
    }
}

/// Convert through the alternatives of the supplied node, when the alternative set names one.
pub(super) fn convert_union(context: &mut super::Context<'_>, value: &mut Value, pointer: &str) {
    let Some(alternatives) = context
        .schema()
        .pointer(pointer)
        .and_then(|schema| schema.get("anyOf"))
        .and_then(Value::as_array)
    else {
        return;
    };
    if alternatives
        .iter()
        .any(|alternative| validates(alternative, value))
    {
        return;
    }
    for (index, alternative) in alternatives.iter().enumerate() {
        let mut candidate = value.clone();
        context.convert_child(&mut candidate, pointer, &format!("anyOf/{index}"));
        if validates(alternative, &candidate) {
            *value = candidate;
            return;
        }
    }
}

/// Select the first constant a supplied value converts to.
fn select(value: &Value, constants: &[Value]) -> Option<Value> {
    constants.iter().find_map(|constant| {
        let mut candidate = value.clone();
        convert_member(&mut candidate, constant);
        equals(&candidate, constant).then_some(candidate)
    })
}

/// Whether a supplied value is the declared constant, comparing numbers by value.
fn equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        _ => left == right,
    }
}

/// Convert a supplied value through the kind its declared constant has.
fn convert_member(value: &mut Value, constant: &Value) {
    match constant {
        Value::String(_) => scalars::to_string(value),
        Value::Number(_) => scalars::to_number(value, false),
        Value::Bool(_) => scalars::to_boolean(value),
        Value::Null => scalars::to_null(value),
        _ => {}
    }
}
