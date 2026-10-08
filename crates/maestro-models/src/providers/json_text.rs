//! JSON text as ECMAScript `JSON.stringify` writes it, and its notion of a present value.

use serde_json::{Map, Number, Value};

/// Serialize without whitespace, keys in canonical order, floats as ECMAScript prints them.
///
/// Keys that are canonical array indices come first in ascending numeric order, then
/// every other key in insertion order. Integers keep their exact digits.
///
/// # Errors
/// Returns the serializer failure when a string cannot be written.
pub(crate) fn compact_json(value: &Value) -> Result<String, serde_json::Error> {
    let mut text = String::new();
    write_value(value, &mut text)?;
    Ok(text)
}

/// Serialize an object exactly as [`compact_json`] serializes a value holding it.
///
/// # Errors
/// Returns the serializer failure when a string cannot be written.
pub(crate) fn compact_object(members: &Map<String, Value>) -> Result<String, serde_json::Error> {
    let mut text = String::new();
    write_object(members, &mut text)?;
    Ok(text)
}

/// Append one value to the output text.
fn write_value(value: &Value, text: &mut String) -> Result<(), serde_json::Error> {
    match value {
        Value::Null => text.push_str("null"),
        Value::Bool(flag) => text.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => write_number(number, text),
        Value::String(string) => text.push_str(&serde_json::to_string(string)?),
        Value::Array(items) => {
            text.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    text.push(',');
                }
                write_value(item, text)?;
            }
            text.push(']');
        }
        Value::Object(members) => write_object(members, text)?,
    }
    Ok(())
}

/// Append a number: exact integer digits, ECMAScript spelling for floats.
fn write_number(number: &Number, text: &mut String) {
    match number.as_f64().filter(|_| number.is_f64()) {
        Some(float) => text.push_str(ryu_js::Buffer::new().format_finite(float)),
        None => text.push_str(&number.to_string()),
    }
}

/// Append an object with array-index keys first.
fn write_object(members: &Map<String, Value>, text: &mut String) -> Result<(), serde_json::Error> {
    let mut indexed: Vec<(u32, &String, &Value)> = Vec::new();
    let mut others: Vec<(&String, &Value)> = Vec::new();
    for (key, value) in members {
        match array_index(key) {
            Some(index) => indexed.push((index, key, value)),
            None => others.push((key, value)),
        }
    }
    indexed.sort_by_key(|(index, _, _)| *index);
    text.push('{');
    let ordered = indexed
        .into_iter()
        .map(|(_, key, value)| (key, value))
        .chain(others);
    for (position, (key, value)) in ordered.enumerate() {
        if position > 0 {
            text.push(',');
        }
        text.push_str(&serde_json::to_string(key)?);
        text.push(':');
        write_value(value, text)?;
    }
    text.push('}');
    Ok(())
}

/// Parse a canonical array index: no sign, no leading zero, below 2^32 - 1.
fn array_index(key: &str) -> Option<u32> {
    let canonical =
        key == "0" || (!key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
    canonical
        .then(|| key.parse::<u32>().ok())
        .flatten()
        .filter(|index| *index != u32::MAX)
}

/// Report whether a value counts as present when used as a condition.
///
/// Null, `false`, zero and the empty string do not; arrays and objects always do.
pub(crate) fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
